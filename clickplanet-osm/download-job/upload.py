import os
import io
import time
from google.cloud import storage

def create_client():
    return storage.Client()


def create_resumable_upload_session(bucket_name, object_name, chunk_size=None):
    """
    Create a resumable upload session for a GCS object
    
    Args:
        bucket_name: Name of the bucket to upload to
        object_name: Name of the object to create
        chunk_size: Optional chunk size for uploads
        
    Returns:
        A tuple of (client, bucket, blob) for the upload
    """
    client = create_client()
    bucket = client.bucket(bucket_name)
    blob = bucket.blob(object_name)
    if chunk_size:
        blob.chunk_size = chunk_size
    
    return client, bucket, blob


def upload_file_to_gcs(bucket_name, local_file_path, object_name, chunk_size=None):
    """
    Upload a file to Google Cloud Storage with proper resumable upload support
    
    Args:
        bucket_name: Name of the GCS bucket
        local_file_path: Path to the local file to upload
        object_name: Destination object name in the bucket
        chunk_size: Optional chunk size for uploads (defaults to GCS client default)
        
    Returns:
        The uploaded blob object
    """

    if not os.path.exists(local_file_path):
        raise FileNotFoundError(f"File not found: {local_file_path}")
    
    client = create_client()
    bucket = client.bucket(bucket_name)
    blob = bucket.blob(object_name)
    if chunk_size:
        blob.chunk_size = chunk_size
    

    blob.upload_from_filename(local_file_path)
    
    return blob


def compose_parts(bucket_name, object_name, part_blobs, delete_parts=False, content_type='application/octet-stream'):
    """
    Compose multiple part objects into a single GCS object using the GCS compose API
    
    Args:
        bucket_name: Name of the GCS bucket
        object_name: Name of the final composed object
        part_blobs: List of blob objects representing the parts to compose
        delete_parts: Whether to delete the part objects after composition
        content_type: Content type for the final object
        
    Returns:
        The composed blob object
    """

    sorted_parts = sorted(part_blobs, key=lambda blob: int(blob.name.split('part')[1].split('.')[0]))
    
    # Create client and get bucket
    client = create_client()
    bucket = client.bucket(bucket_name)
    

    destination_blob = bucket.blob(object_name)
    destination_blob.content_type = content_type
    

    destination_blob.compose(sorted_parts)
    

    if delete_parts:
        for part_blob in part_blobs:
            part_blob.delete()
            
    return destination_blob

def upload_file_part_to_gcs(bucket_name, object_name, data, part_number, content_type='application/octet-stream'):
    """
    Upload a part of data as part of a multipart upload
    
    Args:
        bucket_name: Name of the GCS bucket
        object_name: Name to give the GCS object
        data: Bytes data to upload
        part_number: Part number (used for generating a temporary object name)
        content_type: Optional content type
        
    Returns:
        The uploaded blob object
    """

    timestamp = int(time.time())
    temp_object_name = f"{object_name}.part{part_number}.{timestamp}"
    
    client = create_client()
    bucket = client.bucket(bucket_name)
    blob = bucket.blob(temp_object_name)
    

    if content_type:
        blob.content_type = content_type
    

    blob.upload_from_string(
        data,
        content_type=content_type,
        timeout=120  # 2 minutes timeout
    )
    
    return blob

