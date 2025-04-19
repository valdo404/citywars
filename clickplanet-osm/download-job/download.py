import os
import time
import traceback
import subprocess
import shutil

import requests

from utils import logger, DEBUG, KB, MB, GB

def cleanup_download_resources():
    logger.debug("No cleanup needed for in-memory downloads")

def get_remote_file_size(url):
    try:
        logger.info(f"Getting file size for {url} using requests")
        
        # Try HEAD request first with redirect following
        logger.debug(f"Trying HEAD request with redirect following")
        response = requests.head(url, timeout=60, allow_redirects=True)
        
        if response.status_code == 200 and 'Content-Length' in response.headers:
            size = int(response.headers['Content-Length'])
            logger.info(f"Remote file size from HEAD: {size/MB:.2f} MB ({size/GB:.2f} GB)")
            return size
            
        headers = {'Range': 'bytes=0-0'}  # Only request the first byte to minimize data transfer
        response = requests.get(url, headers=headers, timeout=60, allow_redirects=True)
        
        if response.status_code == 206 and 'Content-Range' in response.headers:
            content_range = response.headers['Content-Range']
            size = int(content_range.split('/')[-1])
            logger.info(f"Remote file size from Range request: {size/MB:.2f} MB ({size/GB:.2f} GB)")
            return size
        
        logger.debug(f"Range request failed, trying full GET to follow redirects and check final URL")
        session = requests.Session()
        response = session.get(url, timeout=60, allow_redirects=True, stream=True)
        
        response.close()
        
        # Check if the final URL has content length
        if response.status_code == 200 and 'Content-Length' in response.headers:
            size = int(response.headers['Content-Length'])
            logger.info(f"Remote file size from final redirect URL: {size/MB:.2f} MB ({size/GB:.2f} GB)")
            return size
        
        logger.error(f"Failed to get Content-Length from server for {url}")
        return None
    except Exception as e:
        logger.error(f"Error getting file size: {str(e)}")
        if DEBUG:
            logger.debug(traceback.format_exc())
        return None



def download_chunk_to_memory(url, start_byte, end_byte, progress_callback=None):
    """Download a chunk directly into memory without saving to a file
    
    Args:
        url: URL to download from
        start_byte: Start byte position
        end_byte: End byte position
        progress_callback: Optional callback function for progress updates
        
    Returns:
        Tuple of (success, result) where result is either the bytes data or error message
    """
    chunk_size = end_byte - start_byte + 1
    logger.info(f"Downloading chunk to memory: bytes {start_byte}-{end_byte} ({chunk_size/MB:.2f} MB)")
    
    try:
        headers = {'Range': f'bytes={start_byte}-{end_byte}'}
        logger.debug(f"Requesting byte range: {headers['Range']}")
        
        # Use requests with allow_redirects to follow redirects
        with requests.get(url, headers=headers, stream=False, timeout=60, allow_redirects=True) as response:
            if response.status_code not in (200, 206):
                error_msg = f"HTTP error: {response.status_code}"
                logger.error(error_msg)
                return False, error_msg
            
            data = response.content
            
            if progress_callback:
                progress_callback(len(data))
            
            logger.info(f"Successfully downloaded {len(data)/MB:.2f} MB to memory")
            return True, data
            
    except Exception as e:
        error_msg = f"Exception during download: {str(e)}"
        logger.error(error_msg)
        if DEBUG:
            logger.debug(traceback.format_exc())
            
        return False, error_msg





def get_osm_config(extract):
    """Get URL and filename for a specific OSM extract"""
    if extract == 'france':
        return {
            'url': 'https://download.geofabrik.de/europe/france-latest.osm.pbf',
            'filename': 'france-latest.osm.pbf'
        }
    elif extract == 'planet':
        return {
            'url': 'https://planet.openstreetmap.org/pbf/planet-latest.osm.pbf',
            'filename': 'planet-latest.osm.pbf'
        }
    else:
        logger.error(f'Error: Invalid extract "{extract}"')
        logger.error('Valid options are "france" or "planet"')
        return None
