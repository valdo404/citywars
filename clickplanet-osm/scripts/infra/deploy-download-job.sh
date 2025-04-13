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
ZONE=${REGION}-b
JOB_NAME="osm-download-job"
DISK_NAME="osm-download-disk"
DISK_SIZE="100" # GB
DOWNLOAD_DIR=${DOWNLOAD_DIR:-"/downloads"}

# Create persistent disk for temporary storage
echo "Creating persistent disk..."
gcloud compute disks create $DISK_NAME \
    --project=$PROJECT_ID \
    --type=pd-balanced \
    --size=${DISK_SIZE}GB \
    --zone=$ZONE || true

# Build and push the container
gcloud builds submit download-job \
    --tag gcr.io/$PROJECT_ID/$JOB_NAME

# Create the Cloud Run job
gcloud run jobs create $JOB_NAME \
    --image gcr.io/$PROJECT_ID/$JOB_NAME \
    --tasks 1 \
    --max-retries 3 \
    --task-timeout 3600 \
    --memory 4Gi \
    --cpu 2 \
    --region $REGION \
    --project $PROJECT_ID \
    --set-env-vars="BUCKET_NAME=${PROJECT_ID}-osm-data,DOWNLOAD_DIR=$DOWNLOAD_DIR" \
    --service-account="$SERVICE_ACCOUNT" \
    --add-volume=name=downloads,type=persistent-disk,disk-name=$DISK_NAME,disk-type=balanced,size=${DISK_SIZE}Gi \
    --add-volume-mount=volume=downloads,mount-path=$DOWNLOAD_DIR

# Execute the job for France
echo "Starting job to download France OSM data..."
gcloud run jobs execute $JOB_NAME \
    --region $REGION \
    --project $PROJECT_ID \
    --update-env-vars="EXTRACT=france"

echo "Job started! Monitor progress with:"
echo "gcloud run jobs executions list --job $JOB_NAME --region $REGION --project $PROJECT_ID"

# Wait for job to complete
echo "Waiting for job to complete..."
while true; do
    status=$(gcloud run jobs executions list \
        --job $JOB_NAME \
        --region $REGION \
        --project $PROJECT_ID \
        --format="get(status)" \
        --limit=1)
    
    if [[ "$status" == "SUCCEEDED" ]]; then
        echo "Job completed successfully"
        break
    elif [[ "$status" == "FAILED" ]]; then
        echo "Job failed"
        break
    fi
    
    echo "Job still running... (status: $status)"
    sleep 30
done

# Clean up persistent disk
echo "Cleaning up persistent disk..."
gcloud compute disks delete $DISK_NAME \
    --project=$PROJECT_ID \
    --zone=$ZONE \
    --quiet
