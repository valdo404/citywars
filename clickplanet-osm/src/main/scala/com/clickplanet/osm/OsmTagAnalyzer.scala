package com.clickplanet.osm

import com.typesafe.scalalogging.LazyLogging
import org.apache.spark.sql.{SparkSession, DataFrame}
import org.apache.spark.sql.functions._
import org.rogach.scallop._

class OsmTagAnalyzerConf(arguments: Seq[String]) extends ScallopConf(arguments) {
  val parquetPath: ScallopOption[String] = opt[String](
    name = "parquet-file",
    descr = "Path to Parquet file containing OSM data",
    required = true
  )
  
  val topN: ScallopOption[Int] = opt[Int](
    name = "top-n",
    descr = "Number of top tags to display",
    default = Some(50)
  )
  
  verify()
}

object OsmTagAnalyzer extends LazyLogging {
  
  def main(args: Array[String]): Unit = {
    val conf = new OsmTagAnalyzerConf(args)
    
    val spark = SparkSession.builder()
      .appName("ClickPlanet OSM Tag Analyzer")
      .getOrCreate()
    
    try {
      val parquetPath = conf.parquetPath()
      val topN = conf.topN()
      
      println(s"Reading OSM data from Parquet: $parquetPath")
      
      // Load the Parquet file
      val osmDF = spark.read.parquet(parquetPath)
      
      println("Data Schema:")
      osmDF.printSchema()
      
      println(s"Total OSM elements: ${osmDF.count()}")
      
      // Break down elements by type
      println("\nElement types distribution:")
      val typeDistDF = osmDF.groupBy("type")
        .count()
        .orderBy(desc("count"))
      
      typeDistDF.show(false)
      
      // Analyze all available tags
      println("\nAnalyzing OSM tags...")
      
      // Get all distinct tag keys
      val tagKeysDF = osmDF
        .filter(col("tags").isNotNull)
        .select(explode(map_keys(col("tags"))).as("tag_key"))
        .groupBy("tag_key")
        .count()
        .orderBy(desc("count"))
      
      println(s"\n=== Top $topN Tag Keys by Frequency ===")
      tagKeysDF.show(topN, false)
      
      println(s"Total distinct tag keys: ${tagKeysDF.count()}")
      
      // Analyze road tags (elements with highway tag)
      println("\n=== Top Highway Tag Values ===")
      osmDF
        .filter(col("tags.highway").isNotNull)
        .groupBy(col("tags.highway").as("highway_type"))
        .count()
        .orderBy(desc("count"))
        .show(topN, false)
      
      // Analyze place tags (elements with place tag)
      println("\n=== Top Place Tag Values ===")
      osmDF
        .filter(col("tags.place").isNotNull)
        .groupBy(col("tags.place").as("place_type"))
        .count()
        .orderBy(desc("count"))
        .show(topN, false)
      
      // Analyze amenity tags
      println("\n=== Top Amenity Tag Values ===")
      osmDF
        .filter(col("tags.amenity").isNotNull)
        .groupBy(col("tags.amenity").as("amenity_type"))
        .count()
        .orderBy(desc("count"))
        .show(topN, false)
      
      // Analyze building tags
      println("\n=== Top Building Tag Values ===")
      osmDF
        .filter(col("tags.building").isNotNull)
        .groupBy(col("tags.building").as("building_type"))
        .count()
        .orderBy(desc("count"))
        .show(topN, false)
      
      // Show tag key-value distribution for the most common tag
      val mostCommonTag = tagKeysDF.select("tag_key").limit(1).collect()(0).getString(0)
      
      println(s"\n=== Top Values for Most Common Tag: '$mostCommonTag' ===")
      osmDF
        .filter(col(s"tags.$mostCommonTag").isNotNull)
        .groupBy(col(s"tags.$mostCommonTag").as(s"${mostCommonTag}_value"))
        .count()
        .orderBy(desc("count"))
        .show(topN, false)
      
    } catch {
      case e: Exception => 
        logger.error(s"Error analyzing OSM Parquet data: ${e.getMessage}")
        e.printStackTrace()
    } finally {
      spark.stop()
    }
  }
}
