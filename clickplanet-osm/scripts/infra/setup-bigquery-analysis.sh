#!/bin/bash

# Setup BigQuery analysis environment for OSM data
PROJECT_ID=${1:-"your-project-id"}
DATASET="osm_dataset"
LOCATION="EU"  # or US

# Create dataset
bq mk --dataset \
  --description "OpenStreetMap Analysis Dataset" \
  --location $LOCATION \
  $PROJECT_ID:$DATASET

# Create external table pointing to Parquet files
bq query --use_legacy_sql=false "
CREATE OR REPLACE EXTERNAL TABLE \`$PROJECT_ID.$DATASET.france\`
OPTIONS (
  format = 'PARQUET',
  uris = ['gs://$PROJECT_ID-osm/france-latest_osm_data.parquet/*.parquet'],
  hive_partition_uri_prefix = 'gs://$PROJECT_ID-osm/france-latest_osm_data.parquet/'
)
"

# Create useful views
bq query --use_legacy_sql=false "
CREATE OR REPLACE VIEW \`$PROJECT_ID.$DATASET.cities\` AS
SELECT 
  tags['name'] as city_name,
  ST_GEOGPOINT(
    CAST(tags['lon'] AS FLOAT64),
    CAST(tags['lat'] AS FLOAT64)
  ) as location,
  tags['population'] as population
FROM \`$PROJECT_ID.$DATASET.france\`
WHERE tags['place'] IN ('city', 'town')
"

# Create sample queries
cat > sample_queries.sql << EOL
-- Find all restaurants in Paris
SELECT 
  tags['name'] as name,
  ST_GEOGPOINT(
    CAST(tags['lon'] AS FLOAT64),
    CAST(tags['lat'] AS FLOAT64)
  ) as location
FROM \`$PROJECT_ID.$DATASET.france\`
WHERE 
  tags['amenity'] = 'restaurant'
  AND ST_DWithin(
    ST_GEOGPOINT(
      CAST(tags['lon'] AS FLOAT64),
      CAST(tags['lat'] AS FLOAT64)
    ),
    ST_GEOGPOINT(2.3522, 48.8566),  -- Paris center
    5000  -- 5km radius
  );

-- Population density by department
SELECT 
  tags['ref'] as department_code,
  tags['name'] as department_name,
  COUNT(*) as num_cities,
  SUM(CAST(tags['population'] AS INT64)) as total_population
FROM \`$PROJECT_ID.$DATASET.france\`
WHERE 
  tags['admin_level'] = '6'  -- French departments
GROUP BY 1, 2
ORDER BY total_population DESC;
EOL

echo "Setup complete! Sample queries available in sample_queries.sql"
