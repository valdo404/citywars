#!/bin/bash

# Dataproc job submission script for OSM processing
# Usage: ./submit-dataproc-job.sh <cluster-name> <region> <project-id> <input-file>

CLUSTER_NAME=${1:-"osm-processing-cluster"}
REGION=${2:-"us-central1"}
PROJECT_ID=${3:-"your-project-id"}
INPUT_FILE=${4:-"planet-latest.osm.pbf"}
BUCKET_NAME="gs://${PROJECT_ID}-dataproc"

# Upload the jar to Cloud Storage
echo "Uploading application jar to Cloud Storage..."
gsutil cp target/scala-2.12/clickplanet-osm-assembly.jar "${BUCKET_NAME}/jars/"

# Submit the Spark job
echo "Submitting Spark job to Dataproc cluster..."
gcloud dataproc jobs submit spark \
    --project=${PROJECT_ID} \
    --region=${REGION} \
    --cluster=${CLUSTER_NAME} \
    --class=com.clickplanet.osm.OsmExtractor \
    --jars="${BUCKET_NAME}/jars/clickplanet-osm-assembly.jar" \
    --properties="spark.executor.memory=32g,\
spark.executor.cores=4,\
spark.driver.memory=16g,\
spark.sql.shuffle.partitions=1000" \
    -- \
    --osm-file="${BUCKET_NAME}/input/${INPUT_FILE}" \
    --partitions=1000

echo "Job submitted successfully. Monitor the progress in Cloud Console or use:"
echo "gcloud dataproc jobs list --region=${REGION} --project=${PROJECT_ID}"
