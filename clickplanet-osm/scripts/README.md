# ClickPlanet OSM Scripts

This directory contains various scripts used in the ClickPlanet OSM project.

## Directory Structure

### Infrastructure Scripts (`infra/`)

Scripts for setting up and managing infrastructure:

- `create-dataproc-cluster.sh` - Creates a Dataproc cluster with optimized settings for OSM processing
- `submit-dataproc-job.sh` - Submits the OSM processing job to a Dataproc cluster
- `setup-bigquery-analysis.sh` - Sets up BigQuery dataset and tables for OSM analysis
- `create-tables.sh` - Creates database tables for storing OSM data

### Utility Scripts (`utils/`)

Helper scripts for development and maintenance:

- `analyze-osm-tags.sh` - Analyzes OSM tags in processed data
- `cleanup-parquet.sh` - Cleans up temporary Parquet files
- `download-france.sh` - Downloads France OSM data for testing

## Usage

Each script has its own usage instructions. Generally, you can run them as:

```bash
# Infrastructure scripts
./scripts/infra/create-dataproc-cluster.sh [cluster-name] [region] [project-id]
./scripts/infra/submit-dataproc-job.sh [cluster-name] [region] [project-id] [input-file]
./scripts/infra/setup-bigquery-analysis.sh [project-id]

# Utility scripts
./scripts/utils/analyze-osm-tags.sh [parquet-path] [top-n]
./scripts/utils/cleanup-parquet.sh
./scripts/utils/download-france.sh
```

Note: Make sure to review and modify any environment-specific values in these scripts before running them.
