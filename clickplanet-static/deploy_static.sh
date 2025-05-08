#!/bin/bash
#
# Move static assets into clickplanet-static/ then deploy to GCS via gcloud CLI.
# Usage:
#   ./deploy_static.sh <GCS_BUCKET>
#   or set GCS_BUCKET env var.
#

set -e

SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
LOCAL_SRC="$SCRIPT_DIR/static"
WEBAPP_SRC="$(cd "$SCRIPT_DIR/../clickplanet-webapp/public" && pwd)/static"

move_static() {
  if [ -d "$LOCAL_SRC" ]; then
    echo "Using existing static in $LOCAL_SRC"
  elif [ -d "$WEBAPP_SRC" ]; then
    echo "Moving static files: $WEBAPP_SRC -> $SCRIPT_DIR"
    mv "$WEBAPP_SRC" "$LOCAL_SRC"
  else
    echo "No static found in '$LOCAL_SRC' or '$WEBAPP_SRC'"
    exit 1
  fi
}

deploy() {
  local bucket="$1"
  
  echo "Using bucket 'gs://$bucket'"
  
  # Configure bucket for website hosting
  echo "Configuring bucket for static website hosting"
  gcloud storage buckets update "gs://$bucket" \
    --web-main-page-suffix=index.html \
    --web-error-page=404.html
  
  # Make bucket contents publicly readable
  echo "Setting public read access"
  gcloud storage buckets add-iam-policy-binding "gs://$bucket" \
    --member=allUsers \
    --role=roles/storage.objectViewer
  
  # Upload files
  echo "Uploading files from $LOCAL_SRC to gs://$bucket/"
  
  # First clean the bucket to avoid old files
  echo "Cleaning existing files in the bucket..."
  gcloud storage objects list "gs://$bucket/" | grep -v "^gs://$bucket/$" | xargs -r gcloud storage rm 2>/dev/null || true
  
  # Use the proper recursive upload with gcloud CLI
  echo "Uploading all files and directories recursively"
  
  # Simply use the recursive upload with appropriate cache-control
  echo "Uploading all files with appropriate cache-control"
  # First remove .DS_Store files
  find "$LOCAL_SRC" -name ".DS_Store" -type f -delete
  
  # Upload everything recursively
  gcloud storage cp "$LOCAL_SRC" "gs://$bucket" --recursive \
    --cache-control="public, max-age=3600"
  
  if [ $? -ne 0 ]; then
    echo "Failed to upload files"
    exit 1
  fi
  
  echo "Successfully deployed static assets to gs://$bucket/"
  echo "Website URL: https://storage.googleapis.com/$bucket/index.html"
}

# Main
BUCKET=${1:-$GCS_BUCKET}
if [ -z "$BUCKET" ]; then
  echo "Usage: ./deploy_static.sh <GCS_BUCKET> or set GCS_BUCKET env var."
  exit 1
fi

move_static
deploy "$BUCKET"
