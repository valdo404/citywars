# ClickPlanet OSM Extractor

A distributed OpenStreetMap (OSM) extractor for ClickPlanet using Apache Spark and osm4scala.

## Overview

This module replaces the previous Rust-based OSM extractor with a Scala/Apache Spark implementation. 
It provides distributed processing capabilities to efficiently parse planet-sized OSM data files and extract
city and road information for the ClickPlanet application.

## Features

- Distributed processing using Apache Spark
- Automatic download of OSM files if not found locally
- Extraction of cities and roads from OSM data
- Storage in PostgreSQL database for use by ClickPlanet server
- Command-line interface for configuration

## Requirements

- Java 8+
- Scala 2.12
- SBT (Scala Build Tool)
- Apache Spark 3.3.2
- PostgreSQL with PostGIS extensions

## Building

```bash
cd clickplanet-osm
sbt assembly
```

This will create a fat JAR file in `target/scala-2.12/clickplanet-osm-0.1.0.jar`.

## Usage

Run the extractor with:

```bash
spark-submit \
  --class com.clickplanet.osm.OsmExtractor \
  --master local[*] \
  target/scala-2.12/clickplanet-osm-0.1.0.jar \
  --osm-file /path/to/planet.osm.pbf \
  --db-url jdbc:postgresql://localhost:5432/clickplanet \
  --db-user postgres \
  --db-password postgres \
  --partitions 16
```

### Parameters

- `--osm-file`: Path to the OSM PBF file to process
- `--db-url`: JDBC URL for PostgreSQL database (default: jdbc:postgresql://localhost:5432/clickplanet)
- `--db-user`: Database username (default: postgres)
- `--db-password`: Database password (default: postgres)
- `--partitions`: Number of Spark partitions to use (default: 16)
- `--download-url`: URL to download OSM file if not found locally (default: Monaco extract)

## Implementation Details

The implementation uses osm4scala's Spark connector to load OSM data directly into Spark DataFrames.
These DataFrames are then filtered and transformed to extract:

1. Cities - From OSM nodes with a "place" tag of "city" or "town"
2. Roads - From OSM ways with a "highway" tag

For full planet files, increase the number of partitions based on your cluster size. A planet file 
is approximately 1.2TB uncompressed, so plan your cluster resources accordingly.

## Development

To develop and test locally, use the Monaco extract which is downloaded automatically
if no OSM file is specified.
