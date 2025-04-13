#!/bin/bash

set -x

# Dataproc cluster creation script for OSM processing
# Usage: ./create-dataproc-cluster.sh <project-id>

CLUSTER_NAME="osm-processing"
REGION="europe-west1"
PROJECT_ID=${1:?"Error: PROJECT_ID (arg 1) is required"}
BUCKET_NAME="gs://${PROJECT_ID}-dataproc"

# Create a Cloud Storage bucket for temporary data
gsutil mb -l ${REGION} ${BUCKET_NAME} || true

# Create the Dataproc cluster with auto zone and spot instances for cost optimization
echo "Creating a Dataproc cluster for OSM processing with auto zone selection and spot instances..."
gcloud dataproc clusters create ${CLUSTER_NAME} \
    --project=${PROJECT_ID} \
    --region=${REGION} \
    --zone=${REGION}-c \
    --enable-component-gateway \
    --master-machine-type=n1-standard-4 \
    --master-boot-disk-type=pd-ssd \
    --master-boot-disk-size=100 \
    --worker-machine-type=n1-highmem-4 \
    --worker-boot-disk-type=pd-ssd \
    --worker-boot-disk-size=100 \
    --num-workers=2 \
    --secondary-worker-type=spot \
    --num-secondary-workers=1 \
    --secondary-worker-boot-disk-type=pd-ssd \
    --secondary-worker-boot-disk-size=100 \
    --image-version=2.2-debian12 \
    --max-idle="2h" \
    --labels=purpose=osm-processing,environment=dev \
    --metadata="OSM_BUCKET=${BUCKET_NAME}"
