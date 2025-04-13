package com.clickplanet.osm

import java.io.File
import java.sql.{Connection, DriverManager, PreparedStatement, Statement}

import com.typesafe.scalalogging.LazyLogging
import org.apache.spark.sql.{SparkSession, DataFrame}
import org.apache.spark.sql.functions._
import org.apache.spark.storage.StorageLevel

import org.rogach.scallop._
import org.locationtech.jts.geom.{Coordinate, GeometryFactory, LineString, Point}
import scala.collection.JavaConverters._
import scala.util.{Try, Success, Failure}

class OsmExtractorConf(arguments: Seq[String]) extends ScallopConf(arguments) {
  val osmFile: ScallopOption[String] = opt[String](
    name = "osm-file",
    descr = "Path to OSM PBF file",
    required = true
  )
  
  val dbUrl: ScallopOption[String] = opt[String](
    name = "db-url",
    descr = "PostgreSQL database URL (jdbc:postgresql://host:port/database)",
    default = Some("jdbc:postgresql://localhost:5432/clickplanet")
  )
  
  val dbUser: ScallopOption[String] = opt[String](
    name = "db-user",
    descr = "PostgreSQL database user",
    default = Some("postgres")
  )
  
  val dbPassword: ScallopOption[String] = opt[String](
    name = "db-password",
    descr = "PostgreSQL database password",
    default = Some("postgres")
  )
  
  val partitions: ScallopOption[Int] = opt[Int](
    name = "partitions",
    descr = "Number of Spark partitions to use",
    default = Some(16)
  )
  
  val downloadUrl: ScallopOption[String] = opt[String](
    name = "download-url",
    descr = "URL to download OSM file from if not found locally",
    default = Some("https://download.geofabrik.de/europe/monaco-latest.osm.pbf")
  )
  
  verify()
}

object OsmExtractor extends LazyLogging {
  import org.apache.spark.sql.functions._
    

  def main(args: Array[String]): Unit = {
    val conf = new OsmExtractorConf(args)
    
    val spark = SparkSession.builder()
      .appName("ClickPlanet OSM Extractor")
      .getOrCreate()
          
    val result = for {
      osmFilePath <- ensureOsmFileExists(
        conf.osmFile.getOrElse(""),
        conf.downloadUrl.getOrElse("https://download.geofabrik.de/europe/monaco-latest.osm.pbf")
      )
      
      _ = println(s"Processing OSM file: $osmFilePath")
      
      _ <- checkDatabaseConnection(conf.dbUrl(), conf.dbUser(), conf.dbPassword())
      
      _ <- Try(processOsmFile(spark, osmFilePath, conf))
    } yield ()
    
    result match {
      case Success(_) => println("OSM Extraction completed successfully")
      case Failure(error) => logger.error(s"OSM Extraction failed: ${error.getMessage}")
    }
    
    spark.stop()
  }
  
  private def ensureOsmFileExists(filePath: String, downloadUrl: String): Try[String] = {
    val file = new File(filePath)
    
    if (file.exists() && file.isFile) {
      Success(file.getAbsolutePath)
    } else {
      val errorMsg = s"OSM file not found at: ${file.getAbsolutePath}. Please download it from: $downloadUrl"
      Failure(new Exception(errorMsg))
    }
  }
  
  private def checkDatabaseConnection(dbUrl: String, user: String, password: String): Try[Unit] = {
    logger.info("Checking database connection...")
    
    for {
      _ <- Try(Class.forName("org.postgresql.Driver"))
      
      conn <- Try(DriverManager.getConnection(dbUrl, user, password))
      
      isValid = conn.isValid(5)

      _ <- Try(conn.close())

      _ <- if (!isValid) Failure(new Exception("Connection validation failed")) else Success(())
    } yield {
      logger.info("Database connection successful. Tables should be created using create-tables.sh script.")
      ()
    }
  }
  

  private def processOsmFile(spark: SparkSession, osmFilePath: String, conf: OsmExtractorConf): Unit = {
    import spark.implicits._
        
    println("Loading OSM data with Spark")
    
    val osmDF = spark.read
      .format("osm.pbf")
      .load(osmFilePath)   
      //.persist(StorageLevel.DISK_ONLY)

    println("OSM DataFrame Schema:")
    osmDF.printSchema()
    
    println("OSM DataFrame Sample:")
    osmDF.show(5)
    
    // Save OSM data to Parquet format
    val parquetOutputPath = osmFilePath.replaceAll("\\.osm\\.pbf$", "_osm_data.parquet")
    println(s"Saving OSM data to Parquet: $parquetOutputPath")
    
    osmDF.write
      .mode("overwrite")
      .parquet(parquetOutputPath)
    
    println("OSM data saved successfully to Parquet format")
    
    // Analyze tags before extraction
    // println("\nAnalyzing OSM tags...")
    
    // Get distinct tag keys across all elements
    // println("\n=== All Distinct Tag Keys ===")
    // val tagKeysDF = osmDF
    //   .select(explode(map_keys(col("tags"))).as("tag_key"))
    //   .distinct()
    //   .orderBy("tag_key")
    
    // tagKeysDF.show(100, false)
    // println(s"Total distinct tag keys: ${tagKeysDF.count()}")
    
    // Analyze road tags (Way elements with highway tag)
    // println("\n=== Distinct Road Tag Values (highway) ===")
    // osmDF
    //  .filter(col("type") === 1.toByte) // Way type
    //   .filter(col("tags.highway").isNotNull)
    //  .select(col("tags.highway").as("highway_type"))
    //  .distinct()
    //  .orderBy("highway_type")
    //  .show(100, false)
      
    // Analyze city tags (Node elements with place tag)
    // println("\n=== Distinct Place Tag Values ===")
    // osmDF
    //  .filter(col("type") === 0.toByte) // Node type
    //  .filter(col("tags.place").isNotNull)
    //  .select(col("tags.place").as("place_type"))
    //  .distinct()
    //  .orderBy("place_type")
    //  .show(100, false)
    
    // println("\nProceeding with data extraction...")
    
    // println("Extracting cities...")
   // val citiesDF = extractCities(osmDF) // Explicit repartitioning
    // val cityCount = citiesDF.count()
    // println(s"Saving ${cityCount} cities to database")
    // saveCitiesToDatabase(citiesDF, conf.dbUrl(), conf.dbUser(), conf.dbPassword())
        
    // println("Extracting roads...")
    // val roadsDF = extractRoads(osmDF) // Explicit repartitioning
    // val roadCount = roadsDF.count()
    // println(s"Saving ${roadCount} roads to database")
    //saveRoadsToDatabase(roadsDF, conf.dbUrl(), conf.dbUser(), conf.dbPassword())
  }
  
  /**
   * Extract city information from OSM data - enhanced for more detailed city information
   */
  private def extractCities(osmDF: DataFrame): DataFrame = {
    // Use Spark session from DataFrame
    import osmDF.sparkSession.implicits._
    
    // Filter for nodes that represent populated places
    // We're interested in cities, towns, villages and other populated areas
    // This is sorted by importance so when we have data at different scales, we can filter
    //val placesOfInterest = Seq("city", "town", "village", "suburb", "district", "borough", "quarter", "neighborhood")
    
    // Create a filter expression for all place types we're interested in
    //val placeFilter = placesOfInterest.map(place => s"tags['place'] = '$place'").mkString(" OR ")
    
    // Filter and extract city data with enhanced metadata
    osmDF
      .filter(col("type") === 0.toByte) // node type is represented as byte value 0 
      //.filter(col("tags").isNotNull) // Ensure tags are not null
      //.filter(expr(placeFilter))
      .withColumn("place_type", expr("tags['place']"))
      .withColumn("population_raw", expr("cast(tags.population as bigint)"))
      .select(
        col("id").as("city_id"),
        coalesce(col("tags.name"), lit("")).as("name"),
        col("latitude").as("lat"),
        col("longitude").as("lon"),
        coalesce(col("population_raw"), lit(0L)).as("population"),
        col("place_type"),
        coalesce(col("tags.admin_level"), lit("")).as("admin_level"),
        coalesce(col("tags.capital"), lit("")).as("capital"),
        coalesce(col("tags.country_code"), coalesce(col("tags.ISO3166-1"), coalesce(col("tags.ISO3166-1:alpha2"), lit("")))).as("country_code"),
        to_json(col("tags")).as("tags_json")
      )
  }
  
  /**
   * Extract road information from OSM data
   */
  private def extractRoads(osmDF: DataFrame): DataFrame = {
    import osmDF.sparkSession.implicits._
    
    osmDF
      .filter(col("type") === 1.toByte) // way type is represented as byte value 1
      //.filter(col("tags").isNotNull) // Ensure tags are not null
      //.filter(expr("tags['highway'] is not null"))
      .select(
        col("id").as("road_id"),
        coalesce(col("tags.name"), lit("")).as("name"),
        col("nodes").as("nodes"),
        to_json(col("tags")).as("tags_json")
      )
  }
  
  /**
   * Save cities to PostgreSQL database using Spark JDBC writer
   */
  
  private def saveCitiesToDatabase(citiesDF: DataFrame, dbUrl: String, user: String, password: String): Unit = {

    // Use the DataFrame API to add a geometry column
    import citiesDF.sparkSession.implicits._
    
    val citiesWithGeomDF = citiesDF
      .withColumn("wkt", concat(lit("POINT("), col("lon"), lit(" "), col("lat"), lit(")"))) 
    
    // Configure JDBC connection properties
    val connectionProperties = new java.util.Properties()
    connectionProperties.setProperty("user", user)
    connectionProperties.setProperty("password", password)
    connectionProperties.setProperty("driver", "org.postgresql.Driver")
    
    // Save DataFrame to PostgreSQL using JDBC writer
    logger.info(s"Saving ${citiesWithGeomDF.count()} cities to database")
    
    // Clear existing data and write new data
    citiesWithGeomDF.write
      .mode("overwrite") // Use overwrite instead of append+truncate
      .jdbc(dbUrl, "cities", connectionProperties)
  }
  
  /**
   * Save roads to PostgreSQL database using Spark JDBC writer
   */
  // We already have createRoadsTableSQL defined at the object level
  
  private def saveRoadsToDatabase(roadsDF: DataFrame, dbUrl: String, user: String, password: String): Unit = {
    
    // Use the DataFrame API to prepare data
    import roadsDF.sparkSession.implicits._
    
    // Convert nodes array to string and add placeholder geometry
    // In a real implementation, we'd look up node coordinates and create a proper LineString
    val roadsWithGeomDF = roadsDF
      .withColumn("nodes_string", array_join(col("nodes"), ","))
      .withColumn("wkt", lit("LINESTRING(0 0, 1 1)"))
    
    // Configure JDBC connection properties
    val connectionProperties = new java.util.Properties()
    connectionProperties.setProperty("user", user)
    connectionProperties.setProperty("password", password)
    connectionProperties.setProperty("driver", "org.postgresql.Driver")
    
    // Save DataFrame to PostgreSQL using JDBC writer
    logger.info(s"Saving ${roadsWithGeomDF.count()} roads to database")
    
    roadsWithGeomDF.write
      .mode("overwrite") // Use overwrite instead of append+truncate
      .jdbc(dbUrl, "roads", connectionProperties)
  }
}
