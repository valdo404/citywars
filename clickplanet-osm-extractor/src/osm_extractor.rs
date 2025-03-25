use clap::Parser;
use deadpool_postgres::{Client, Config, ManagerConfig, Object, PoolError, RecyclingMethod};
use osmpbf::{Blob, BlobDecode, BlobReader, Node, TagIter, Way};
use rayon::iter::ParallelIterator;
use rayon::iter::ParallelBridge;
use serde_json;
use std::fs::File;
use std::io::BufReader;
use thiserror::Error;
use tokio_postgres::{Config as PgConfig, NoTls};

struct City {
    id: i64,
    name: Option<String>,
    lat: f64,
    lon: f64,
    population: Option<i64>,
}

struct Road {
    id: i64,
    name: Option<String>,
    nodes: Vec<i64>, // List of node IDs forming the road
}


#[derive(Parser)]
#[command(name = "OSM Extractor")]
#[command(author = "Laurent Valdes")]
#[command(version = "1.0")]
#[command(about = "Extract data from OpenStreetMap and save it to a Postgres/PostGIS database")]
struct Args {
    #[arg(short, long)]
    osm_file: String,

    #[arg(short, long)]
    db_url: String,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args = Args::parse();

    extract_osm_to_db(&args.osm_file, &args.db_url).await?;

    println!("Extraction complete. Data saved to Postgres/PostGIS database.");
    Ok(())
}

async fn extract_osm_to_db(osm_file: &str, db_url: &str) -> Result<(), OsmError> {
    let pg_config: &mut tokio_postgres::Config = PgConfig::new().host("localhost").user("user").password("password").dbname("osm"); // Configure
    let mut cfg: Config = Config::new();
    cfg.dbname = Some("osm".to_string());
    cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Fast });
    let pool = cfg.create_pool(None, NoTls).unwrap();

    let mut client = pool.get().await?;
    create_cities_table(&mut client).await?;
    create_roads_table(&mut client).await?;

    let reader: BlobReader<BufReader<File>> = BlobReader::from_path(osm_file)?;


    reader.par_bridge().for_each(|blob| {
        let copied_pool = pool.clone();

        tokio::spawn(async move {  // Spawn a Tokio task for each blob
            let result: Result<(), OsmError> = {
                let blob = blob.map_err(OsmError::from).unwrap();
                let mut client = copied_pool.get().await.unwrap(); // corrected here
                decode_blob(&mut client, blob).await
            };
            if let Err(e) = result {
                eprintln!("Error processing blob: {}", e); // Or better error handling
            }
        });
    });

    Ok(())
}

#[derive(Error, Debug)]
enum OsmError {
    #[error("Decoding error: {0}")]
    DecodeError(String),
    #[error("Pool error: {0}")]
    PoolError(#[from] PoolError),
    #[error("OSM error: {0}")]
    OsmError(#[from] osmpbf::Error),
    #[error("Database error: {0}")]
    DatabaseError(#[from] postgres::Error),
    #[error("Serde JSON error: {0}")]
    SerdeJsonError(#[from] serde_json::Error),
    #[error("I/O error: {0}")]
    IoError(#[from] std::io::Error),
}

async fn decode_blob(client: &mut Client, blob: Blob) -> Result<(), OsmError> {
    let result: Result<BlobDecode, OsmError> = blob.decode().map_err(OsmError::from);
    if let BlobDecode::OsmData(block) = result? {
        for group in block.groups() {
            // Process cities (nodes with "place=city" or "place=town")
            for node in group.nodes() {
                decode_node(client, node).await?;
            }

            // Process roads (ways with "highway" tag)
            for way in group.ways() {
                decode_way(client, way).await?;
            }
        }
    }
    Ok(())
}

async fn decode_way(client: &mut Object, way: Way<'_>) -> Result<(), OsmError> {
    let mut tags: TagIter = way.tags();

    if tags.any(|(key, _)| key == "highway") {
        let mut name = None;
        let nodes = way.refs().collect::<Vec<i64>>();

        for (key, value) in way.tags() { // Iterate using tuple destructuring
            if key == "name" {
                name = Some(value.to_string());
                break;
            }
        }

        let road = Road {
            id: way.id(),
            name,
            nodes,
        };

        insert_road(client, &road).await?;
    };


    Ok(())
}



async fn decode_node(client: &mut Object, node: Node<'_>) -> Result<(), OsmError> {
    let tags = node.tags();

    let mut place_tag = None;
    let mut name_tag = None;
    let mut population_tag = None;

    for (key, val) in tags {
        match key {
            "place" => place_tag = Some(val),
            "name" => name_tag = Some(val),
            "population" => population_tag = Some(val),
            _ => {}, // Ignore other tags
        }
    }

    if let (Some(place), Some(name)) = (place_tag, name_tag) {
        if place == "city" || place == "town" {
            let city = City {
                id: node.id(),
                name: Some(name.to_string()),
                lat: node.lat(),
                lon: node.lon(),
                population: population_tag.and_then(|p| p.parse().ok()),
            };

            insert_city(client, &city).await?;
        }
    }

    Ok(())
}


async fn create_roads_table(client: &mut Client) -> Result<(), OsmError> {
    client.execute(
        "CREATE TABLE IF NOT EXISTS roads (
            road_id BIGINT PRIMARY KEY,
            name TEXT,
            nodes JSONB
        )",
        &[],
    ).await?;
    Ok(())
}

async fn create_cities_table(client: &mut Client) -> Result<(), OsmError> {
    client.execute(
        "CREATE TABLE IF NOT EXISTS cities (
            city_id BIGINT PRIMARY KEY,
            name TEXT,
            latitude DOUBLE PRECISION,
            longitude DOUBLE PRECISION,
            population BIGINT,
            geom GEOGRAPHY(Point, 4326)
        )",
        &[],
    ).await?;
    Ok(())
}

async fn insert_road(client: &mut Object, road: &Road) -> Result<(), OsmError> {
    client.query(
        "INSERT INTO roads (road_id, name, nodes)
                                 VALUES ($1, $2, $3)",
        &[
            &road.id,
            &road.name,
            &serde_json::to_string(&road.nodes)?,
        ],
    ).await?;

    Ok(())
}

async fn insert_city(client: &mut Client, city: &City) -> Result<(), OsmError> {
    client.execute(
        "INSERT INTO cities (city_id, name, latitude, longitude, population, geom)
                                     VALUES ($1, $2, $3, $4, $5, ST_SetSRID(ST_MakePoint($3, $4), 4326))",
        &[
            &city.id,
            &city.name,
            &city.lat,
            &city.lon,
            &city.population,
        ],
    ).await?;
    Ok(())
}
