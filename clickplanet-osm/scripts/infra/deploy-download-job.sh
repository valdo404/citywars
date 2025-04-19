#!/bin/bash
set -e
set -x

if [ -z "$PROJECT_ID" ]; then
    echo "Error: PROJECT_ID environment variable is required"
    exit 1
fi

if [ -z "$SERVICE_ACCOUNT" ]; then
    echo "Warning: SERVICE_ACCOUNT environment variable is not set. Using default compute service account."
    SERVICE_ACCOUNT_PARAM=""
else
    SERVICE_ACCOUNT_PARAM="--service-account=$SERVICE_ACCOUNT"
fi

REGION=${REGION:-"europe-west1"}
JOB_NAME="osm-download-job"
BUCKET_NAME="${PROJECT_ID}-osm-data"

cleanup() {
    echo "Error occurred. Cleaning up resources..."
    exit 1
}

trap cleanup ERR

echo "Building and pushing container image..."
gcloud builds submit $(dirname "$0")/../../download-job \
    --tag gcr.io/$PROJECT_ID/$JOB_NAME

if [ $? -ne 0 ]; then
    echo "Error: Failed to build container image"
    cleanup
fi

echo "Checking if Cloud Run job already exists..."
if gcloud beta run jobs describe $JOB_NAME --region=$REGION --project=$PROJECT_ID &>/dev/null; then
    echo "Updating existing Cloud Run job..."
    gcloud beta run jobs update $JOB_NAME \
        --image gcr.io/$PROJECT_ID/$JOB_NAME \
        --tasks 1 \
        --max-retries 1 \
        --task-timeout 7200 \
        --memory 2Gi \
        --cpu 2 \
        --region $REGION \
        --project $PROJECT_ID \
        --set-env-vars="BUCKET_NAME=$BUCKET_NAME" \
        $SERVICE_ACCOUNT_PARAM
else
    echo "Creating new Cloud Run job..."
    gcloud beta run jobs create $JOB_NAME \
        --image gcr.io/$PROJECT_ID/$JOB_NAME \
        --tasks 1 \
        --max-retries 1 \
        --task-timeout 7200 \
        --memory 2Gi \
        --cpu 2 \
        --region $REGION \
        --project $PROJECT_ID \
        --set-env-vars="BUCKET_NAME=$BUCKET_NAME" \
        $SERVICE_ACCOUNT_PARAM
fi

if [ $? -ne 0 ]; then
    echo "Error: Failed to create Cloud Run job"
    cleanup
fi

echo "All operations completed successfully!"
