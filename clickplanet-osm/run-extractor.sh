#!/bin/bash

# Add PostgreSQL binaries to PATH
export PATH="/opt/homebrew/opt/postgresql@17/bin:$PATH"

# Build the project if needed
if [ ! -f "target/scala-2.12/clickplanet-osm-0.1.0.jar" ]; then
  echo "Building project..."
  sbt assembly
fi

# Default OSM file (Monaco extract if not specified)
OSM_FILE=${1:-"../data/monaco-latest.osm.pbf"}

# Get the file size in MB to determine memory allocation
FILE_SIZE_MB=$(du -m "$OSM_FILE" | cut -f1)
echo "Processing OSM file of size: $FILE_SIZE_MB MB"

# Set combined memory limit for local mode (driver + executor on same machine)
DRIVER_MEMORY="512m"
EXECUTOR_MEMORY="8GB"  # Increased executor memory for sorting operations

# Calculate reasonable number of partitions - too many cause memory issues with sorting
PARTITIONS=256  # Use fewer partitions to reduce memory pressure during shuffle

echo "Allocating driver memory: $DRIVER_MEMORY, executor memory: $EXECUTOR_MEMORY, partitions: $PARTITIONS"

# Run the extractor with Spark 3.1.1 with optimized settings - tuned for sorting large datasets
/Users/laurentvaldes/Projects/clickplanet-client/spark-3.1.1-bin-hadoop3.2/bin/spark-submit \
  --class com.clickplanet.osm.OsmExtractor \
  --master local[2] \
  --driver-memory $DRIVER_MEMORY \
  --executor-memory $EXECUTOR_MEMORY \
  --conf spark.sql.shuffle.partitions=$PARTITIONS \
  --conf spark.memory.fraction=0.8 \
  --conf spark.memory.storageFraction=0.1 \
  --conf spark.shuffle.spill.compress=true \
  --conf spark.shuffle.compress=true \
  --conf spark.io.compression.codec=lz4 \
  --conf spark.shuffle.file.buffer=1m \
  --conf spark.unsafe.sorter.spill.read.ahead.enabled=false \
  --conf spark.speculation=false \
  --packages com.acervera.osm4scala:osm4scala-spark3-shaded_2.12:1.0.11 \
  target/scala-2.12/clickplanet-osm-0.1.0.jar \
  --osm-file "$OSM_FILE" \
  --db-url jdbc:postgresql://localhost:5432/clickplanet \
  --db-user laurentvaldes \
  --db-password "" \
  --partitions $PARTITIONS
