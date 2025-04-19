#!/bin/bash
set -e

# Dataproc job submission script for OSM processing
# Usage: ./submit-dataproc-job.sh <project-id> <input-file> <output-path>

CLUSTER_NAME="osm-processing"
REGION="europe-west1"
PROJECT_ID=${1:?"Error: PROJECT_ID (arg 1) is required"}
INPUT_FILE=${2:-"planet-latest.osm.pbf"}
OUTPUT_PATH=${3:-"processed-data"}
OSM_BUCKET="gs://${PROJECT_ID}-osm-data"
DATAPROC_BUCKET="gs://${PROJECT_ID}-dataproc"

# Ensure the OSM data is accessible to Dataproc
echo "Checking if OSM data is accessible..."
if gsutil -q stat "${OSM_BUCKET}/osm_data/${INPUT_FILE}"; then
    echo "Found file at ${OSM_BUCKET}/osm_data/${INPUT_FILE}"
    INPUT_PATH="${OSM_BUCKET}/osm_data/${INPUT_FILE}"
else
    if gsutil -q stat "${OSM_BUCKET}/input/${INPUT_FILE}"; then
        echo "Found file at ${OSM_BUCKET}/input/${INPUT_FILE}"
        INPUT_PATH="${OSM_BUCKET}/input/${INPUT_FILE}"
    else
        # Check if file exists in the root of the bucket
        if gsutil -q stat "${OSM_BUCKET}/${INPUT_FILE}"; then
            echo "Found file at ${OSM_BUCKET}/${INPUT_FILE}"
            INPUT_PATH="${OSM_BUCKET}/${INPUT_FILE}"
        else
            echo "Error: Could not locate ${INPUT_FILE} in the OSM bucket."
            echo "Checked paths:"
            echo "- ${OSM_BUCKET}/osm_data/${INPUT_FILE}"
            echo "- ${OSM_BUCKET}/input/${INPUT_FILE}"
            echo "- ${OSM_BUCKET}/${INPUT_FILE}"
            exit 1
        fi
    fi
fi

# Download and stage external jars to GCS to avoid HTTP fetch in cluster
echo "Staging external jars to GCS..."
JAR_TMPDIR=$(mktemp -d)
curl -sSL https://repo1.maven.org/maven2/io/github/valdo404/osm4scala-spark3_2.12/1.0.11/osm4scala-spark3_2.12-1.0.11.jar -o "$JAR_TMPDIR/osm4scala-spark3_2.12-1.0.11.jar"
curl -sSL https://repo1.maven.org/maven2/io/github/valdo404/osm4scala-core_2.12/1.0.11/osm4scala-core_2.12-1.0.11.jar -o "$JAR_TMPDIR/osm4scala-core_2.12-1.0.11.jar"
gsutil -q cp "$JAR_TMPDIR"/*.jar "${DATAPROC_BUCKET}/jars/"
rm -rf "$JAR_TMPDIR"

# Submit the Spark job with cost-effective settings
echo "Submitting Spark job to Dataproc cluster ${CLUSTER_NAME}..."
gcloud dataproc jobs submit spark \
    --project=${PROJECT_ID} \
    --region=${REGION} \
    --cluster=${CLUSTER_NAME} \
    --class=com.clickplanet.osm.OsmExtractor \
    --jars="${DATAPROC_BUCKET}/jars/clickplanet-osm-0.1.0.jar,${DATAPROC_BUCKET}/jars/osm4scala-spark3_2.12-1.0.11.jar,${DATAPROC_BUCKET}/jars/osm4scala-core_2.12-1.0.11.jar" \
    --properties="spark.executor.memory=18g,spark.executor.cores=4,spark.driver.memory=4g,spark.yarn.executor.memoryOverhead=4g,spark.sql.shuffle.partitions=1000,spark.local.dir=/mnt/1/spark,spark.shuffle.service.enabled=true,spark.executor.heartbeatInterval=60s,spark.network.timeout=300s,spark.shuffle.consolidateFiles=true,spark.shuffle.file.buffer=128k,spark.reducer.maxSizeInFlight=192m,spark.memory.fraction=0.75,spark.memory.storageFraction=0.3,spark.sql.adaptive.enabled=true,spark.sql.adaptive.coalescePartitions.enabled=true,spark.serializer=org.apache.spark.serializer.KryoSerializer,spark.kryoserializer.buffer=64m,spark.kryoserializer.buffer.max=256m,spark.kryo.registrationRequired=false,spark.shuffle.compress=true,spark.shuffle.spill.compress=true,spark.io.compression.codec=lz4,spark.io.compression.lz4.blockSize=64k,spark.rdd.compress=true,spark.broadcast.compress=true" \
    -- \
    --osm-file="${INPUT_PATH}" \
    --output-path="${OSM_BUCKET}/${OUTPUT_PATH}" \
    --partitions=1000

echo "Job submitted successfully!"
echo ""
echo "Monitor job progress:"
echo "gcloud dataproc jobs list --region=${REGION} --project=${PROJECT_ID}"
echo ""
echo "View output data once job completes:"
echo "gsutil ls ${OSM_BUCKET}/${OUTPUT_PATH}/"
echo ""
echo "To delete the cluster when done:"
echo "gcloud dataproc clusters delete ${CLUSTER_NAME} --region=${REGION} --project=${PROJECT_ID} --quiet"
