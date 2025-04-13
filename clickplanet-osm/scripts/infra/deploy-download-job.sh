#!/bin/bash
set -x

# Required environment variables
if [ -z "$PROJECT_ID" ]; then
    echo "Error: PROJECT_ID environment variable is required"
    exit 1
fi

if [ -z "$SERVICE_ACCOUNT" ]; then
    echo "Error: SERVICE_ACCOUNT environment variable is required"
    exit 1
fi

REGION=${REGION:-"europe-west1"}
JOB_NAME="osm-download-job"

# Build and push the container
gcloud builds submit download-job \
    --tag gcr.io/$PROJECT_ID/$JOB_NAME

# Create the Cloud Run job
gcloud run jobs create $JOB_NAME \
    --image gcr.io/$PROJECT_ID/$JOB_NAME \
    --tasks 1 \
    --max-retries 3 \
    --task-timeout 3600 \
    --memory 2Gi \
    --cpu 1 \
    --region $REGION \
    --project $PROJECT_ID \
    --set-env-vars="BUCKET_NAME=${PROJECT_ID}-osm-data" \
    --service-account="$SERVICE_ACCOUNT"

# Execute the job for France
echo "Starting job to download France OSM data..."
gcloud run jobs execute $JOB_NAME \
    --region $REGION \
    --project $PROJECT_ID \
    --update-env-vars="EXTRACT=france"

echo "Job started! Monitor progress with:"
echo "gcloud run jobs executions list --job $JOB_NAME --region $REGION --project $PROJECT_ID"
