#!/bin/bash

# Dataproc cluster creation script for OSM processing
# Usage: ./create-dataproc-cluster.sh <cluster-name> <region> <project-id>

CLUSTER_NAME=${1:-"osm-processing-cluster"}
REGION=${2:-"us-central1"}
PROJECT_ID=${3:-"your-project-id"}
BUCKET_NAME="gs://${PROJECT_ID}-dataproc"

# Create a Cloud Storage bucket for temporary data
gsutil mb -l ${REGION} ${BUCKET_NAME} || true

# Create the Dataproc cluster
gcloud dataproc clusters create ${CLUSTER_NAME} \
    --project=${PROJECT_ID} \
    --region=${REGION} \
    --zone=${REGION}-a \
    --master-machine-type=n2-standard-8 \
    --master-boot-disk-size=100GB \
    --num-workers=4 \
    --worker-machine-type=n2-highmem-16 \
    --worker-boot-disk-size=100GB \
    --image-version=2.1 \
    --properties="spark:spark.dynamicAllocation.enabled=true,\
spark:spark.dynamicAllocation.initialExecutors=5,\
spark:spark.dynamicAllocation.minExecutors=5,\
spark:spark.dynamicAllocation.maxExecutors=20,\
spark:spark.executor.memory=32g,\
spark:spark.executor.cores=4,\
spark:spark.driver.memory=16g,\
spark:spark.memory.fraction=0.8,\
spark:spark.memory.storageFraction=0.3,\
spark:spark.sql.shuffle.partitions=1000,\
spark:spark.serializer=org.apache.spark.serializer.KryoSerializer,\
spark:spark.kryoserializer.buffer.max=1g,\
spark:spark.speculation=true,\
dataproc:dataproc.conscrypt.provider.enable=false" \
    --optional-components=JUPYTER \
    --enable-component-gateway \
    --max-idle="3h" \
    --scopes=cloud-platform \
    --labels=purpose=osm-processing
