#!/bin/bash
# Script to compile the OSM extractor with minimal output

cd "$(dirname "$0")/../.."

echo "Compiling OSM extractor..."
sbt -Dsbt.log.noformat=true clean compile assembly

if [ $? -eq 0 ]; then
  JAR_PATH="$(pwd)/target/scala-2.12/clickplanet-osm-0.1.0.jar"
  if [ -f "$JAR_PATH" ]; then
    echo "✅ Compilation successful"
    echo "JAR created at: $JAR_PATH"
    echo "Ready to submit to Dataproc"
  else
    echo "❌ Compilation completed but JAR file not found at expected location"
    echo "Expected: $JAR_PATH"
    exit 1
  fi
else
  echo "❌ Compilation failed"
  exit 1
fi
