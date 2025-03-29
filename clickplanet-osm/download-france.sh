#!/bin/bash

# Create data directory if it doesn't exist
mkdir -p ../data

# Download France OSM extract (about 3.8GB) from Geofabrik
echo "Downloading France OSM data (this will take some time)..."
curl -o ../data/france-latest.osm.pbf https://download.geofabrik.de/europe/france-latest.osm.pbf

if [ $? -eq 0 ]; then
  echo "Download complete: ../data/france-latest.osm.pbf"
  echo "File size: $(du -h ../data/france-latest.osm.pbf | cut -f1)"
else
  echo "Download failed!"
  exit 1
fi
