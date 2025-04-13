package com.clickplanet.osm

import com.typesafe.scalalogging.LazyLogging
import org.apache.spark.sql.{SparkSession, DataFrame}
import org.apache.spark.sql.functions._
import org.apache.spark.storage.StorageLevel

import org.rogach.scallop._
import org.locationtech.jts.geom.{Coordinate, GeometryFactory, LineString, Point}
import scala.util.Try

class OsmExtractorConf(arguments: Seq[String]) extends ScallopConf(arguments) {
  val osmFile: ScallopOption[String] = opt[String](
    name = "osm-file",
    descr = "Path to OSM PBF file (can be GCS URI)",
    required = true
  )
  
  val outputPath: ScallopOption[String] = opt[String](
    name = "output-path",
    descr = "Path where extracted data will be saved (GCS or local)",
    required = true
  )
  
  val partitions: ScallopOption[Int] = opt[Int](
    name = "partitions",
    descr = "Number of Spark partitions to use",
    default = Some(16)
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
          
    logger.info(s"Processing OSM file: ${conf.osmFile()}")
    logger.info(s"Output will be saved to: ${conf.outputPath()}")
    logger.info(s"Using ${conf.partitions()} partitions")
      
    try {
      processOsmFile(spark, conf.osmFile(), conf)
      logger.info("OSM Extraction completed successfully")
    } catch {
      case e: Exception => 
        logger.error(s"OSM Extraction failed: ${e.getMessage}")
        spark.stop()
        throw e
    }
    
    spark.stop()
  }
  

  private def processOsmFile(spark: SparkSession, osmFilePath: String, conf: OsmExtractorConf): Unit = {
    import spark.implicits._
        
    logger.info("Loading OSM data with Spark")
    
    // Read OSM data with Spark
    logger.info(s"Reading with format osm.pbf")
    val osmDF = spark.read
      .format("osm.pbf")
      .load(osmFilePath)
      .repartition(conf.partitions())
      .persist(StorageLevel.MEMORY_AND_DISK)

    logger.info("OSM DataFrame Schema:")
    osmDF.printSchema()
    
    // Save OSM data to Parquet format
    val parquetOutputPath = conf.outputPath()
    logger.info(s"Saving OSM data to Parquet: $parquetOutputPath")
    
    // Extract cities and roads
    logger.info("Extracting city data...")
    val citiesDF = extractCities(osmDF)
    
    logger.info("Extracting road data...")
    val roadsDF = extractRoads(osmDF)
    
    // Save raw OSM data to Parquet format
    logger.info(s"Saving raw OSM data to: ${parquetOutputPath}/raw")
    osmDF.write
      .mode("overwrite")
      .parquet(s"${parquetOutputPath}/raw")
      
    // Save processed cities data
    logger.info(s"Saving cities data to: ${parquetOutputPath}/cities")
    citiesDF.write
      .mode("overwrite")
      .parquet(s"${parquetOutputPath}/cities")
      
    // Save processed roads data
    logger.info(s"Saving roads data to: ${parquetOutputPath}/roads")
    roadsDF.write
      .mode("overwrite")
      .parquet(s"${parquetOutputPath}/roads")
    
    // Release memory
    osmDF.unpersist()
    
    logger.info("OSM data saved successfully to Parquet format")
  }
  
  /**
   * Extract city information from OSM data
   */
  private def extractCities(osmDF: DataFrame): DataFrame = {
    // Use Spark session from DataFrame
    import osmDF.sparkSession.implicits._
    
    // Filter and extract city data
    osmDF
      .filter(col("type") === 0.toByte) // node type is represented as byte value 0 
      .filter(col("tags.place").isNotNull) // Only places
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
      .filter(col("tags.highway").isNotNull) // Only roads/highways
      .select(
        col("id").as("road_id"),
        coalesce(col("tags.name"), lit("")).as("name"),
        col("nodes").as("nodes"),
        to_json(col("tags")).as("tags_json")
      )
  }
}
