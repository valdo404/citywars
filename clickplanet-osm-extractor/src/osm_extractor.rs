use std::fs::File;
use std::io::BufReader;
use clap::Parser;
use deadpool_postgres::{Client, PoolError};
use osmpbf::{Blob, BlobDecode, BlobReader, Node, TagIter, Way};
use serde_json;
use thiserror::Error;
use tokio_postgres::{Config as PgConfig, NoTls};

use log::{error, warn, info};
use std::sync::atomic::{AtomicUsize, Ordering};

use tracing_subscriber::{fmt, layer::SubscriberExt, EnvFilter, Registry};
use futures::stream::unfold;
use futures::StreamExt;
use rayon::iter::{IntoParallelRefIterator, ParallelBridge};
use rayon::iter::ParallelIterator;
use rayon::prelude::ParallelSliceMut;

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

fn init_logger() {
    let fmt_layer = fmt::layer()
        .with_target(false) // Disable target display
        .with_thread_ids(true) // Include thread IDs
        .with_thread_names(true) // Include thread names
        .with_file(true) // Include file names
        .with_line_number(true); // Include line numbers

    let env_filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new("info"));

    let subscriber = Registry::default()
        .with(env_filter)
        .with(fmt_layer);

    tracing_log::LogTracer::init().unwrap();
    tracing::subscriber::set_global_default(subscriber).unwrap();
}

#[derive(Parser)]
#[command(name = "OSM Extractor")]
#[command(author = "Laurent Valdes")]
#[command(version = "1.0")]
#[command(about = "Extract data from OpenStreetMap and save it to a Postgres/PostGIS database")]
struct Args {
    #[arg(short, long, default_value = "test")]
    osm_file: String,

    #[arg(short, long, default_value = "test")]
    db_url: String,
}

#[tokio::main(flavor = "multi_thread", worker_threads = 10)]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    init_logger();

    let args = Args::parse();

    extract_osm_to_db(&args.osm_file, &args.db_url).await?;

    println!("Extraction complete. Data saved to Postgres/PostGIS database.");
    Ok(())
}

async fn extract_osm_to_db(osm_file: &str, db_url: &str) -> Result<(), OsmError> {
    //let pg_config: &mut tokio_postgres::Config = PgConfig::new().host("localhost").user("user").password("password").dbname("osm"); // Configure
    //let mut cfg: Config = Config::new();
    //cfg.dbname = Some("osm".to_string());
    //cfg.manager = Some(ManagerConfig { recycling_method: RecyclingMethod::Fast });

    //let pool: Pool = cfg.create_pool(None, NoTls).unwrap();

    let mut v: Vec<_> = (0..100).collect();
    v.par_sort();

    println!("{:?} {:?}", v.par_iter().sum::<i32>(), 100 * 99 / 2);

    // info!("Start of extraction");
    // let reader: BlobReader<BufReader<File>> = BlobReader::from_path(osm_file)?;
    // reader.par_iter().for_each(|blob| async move {
    //     match decode_blob((), blob.expect("no blob")).await {
    //         Ok(_) => tracing::info!("Blob decoded"),
    //         Err(e) => tracing::error!("Error decoding blob: {}", e),
    //     }
    // });

    // let stream = unfold(reader, |mut reader| async move {
    //     match reader.next().transpose() {
    //         Ok(Some(blob)) => Some((blob, reader)), // Produce one blob at a time (stream of items)
    //         _ => None,
    //     }
    // });
    //
    // stream
    //     .for_each_concurrent(8, |blob| async move {
    //         match decode_blob((), blob).await {
    //             Ok(_) => tracing::info!("Blob decoded"),
    //             Err(e) => tracing::error!("Error decoding blob: {}", e),
    //         }
    //     })
    //     .await;



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

async fn decode_blob(client: (), blob: Blob) -> Result<(), OsmError> {
    let result: Result<BlobDecode, OsmError> = blob.decode().map_err(OsmError::from);

    match result {
        Ok(BlobDecode::OsmData(block)) => {
            info!("Decoding blob.");

            for group in block.groups() {
                let mut way_count = 0;
                let mut node_count = 0;
                let mut dense_node_count = 0;
                let mut relation_count = 0;


                for group in block.groups() {
                    way_count += group.ways().count();
                    node_count += group.nodes().count();
                    dense_node_count += group.dense_nodes().count();
                    relation_count += group.relations().count();
                }
                info!(
                    "Blob decoded: ways={}, nodes={}, dense_nodes={}, relations={}",
                    way_count,
                    node_count,
                    dense_node_count,
                    relation_count
                );

                for node in group.nodes() {
                    let node_id = node.id();
                    info!("Decoding node: {:?}", node_id);
                    if let Err(e) = decode_node(client, node).await {
                        warn!("Failed to decode node {}: {}", node_id, e);
                    }
                }

                // Process roads (ways with "highway" tag)
                for way in group.ways() {
                    let way_id = way.id();

                    info!("Decoding way: {:?}", way_id);
                    if let Err(e) = decode_way((), way).await {
                        warn!("Failed to decode way {}: {}", way_id, e);
                    }
                }
            }
        }
        Ok(BlobDecode::OsmHeader(_)) => {
            info!("Decoding header.");

        }
        Ok(BlobDecode::Unknown(_)) => {
        }
        Err(e) => {
            error!("Failed to decode blob: {}", e);
            return Err(e);
        }
    }

    Ok(())
}

async fn decode_way(client: (), way: Way<'_>) -> Result<(), OsmError> {
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



async fn decode_node(client: (), node: Node<'_>) -> Result<(), OsmError> {
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
    // client.execute(
    //     "CREATE TABLE IF NOT EXISTS roads (
    //         road_id BIGINT PRIMARY KEY,
    //         name TEXT,
    //         nodes JSONB
    //     )",
    //     &[],
    // ).await?;
    Ok(())
}

async fn create_cities_table(client: &mut Client) -> Result<(), OsmError> {
    // client.execute(
    //     "CREATE TABLE IF NOT EXISTS cities (
    //         city_id BIGINT PRIMARY KEY,
    //         name TEXT,
    //         latitude DOUBLE PRECISION,
    //         longitude DOUBLE PRECISION,
    //         population BIGINT,
    //         geom GEOGRAPHY(Point, 4326)
    //     )",
    //     &[],
    // ).await?;
    Ok(())
}

async fn insert_road(client: (), road: &Road) -> Result<(), OsmError> {
    // client.query(
    //     "INSERT INTO roads (road_id, name, nodes)
    //                              VALUES ($1, $2, $3)",
    //     &[
    //         &road.id,
    //         &road.name,
    //         &serde_json::to_string(&road.nodes)?,
    //     ],
    // ).await?;

    Ok(())
}

async fn insert_city(client: (), city: &City) -> Result<(), OsmError> {
    // client.execute(
    //     "INSERT INTO cities (city_id, name, latitude, longitude, population, geom)
    //                                  VALUES ($1, $2, $3, $4, $5, ST_SetSRID(ST_MakePoint($3, $4), 4326))",
    //     &[
    //         &city.id,
    //         &city.name,
    //         &city.lat,
    //         &city.lon,
    //         &city.population,
    //     ],
    // ).await?;
    Ok(())
}
