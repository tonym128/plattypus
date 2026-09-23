#!/usr/bin/env python3
"""
Create PS1 STR (MDEC video) file from MJPEG frames.
STR format: Sector-based CD-XA Mode 2 Form 2 sectors.
Each sector: 2352 bytes (raw) with subheader.
"""

import os
import struct
from PIL import Image
import io

# PS1 STR constants
SECTOR_SIZE = 2352
DATA_SIZE = 2048
SUBHEADER_SIZE = 8  # file_num(2) + channel(1) + submode(1) + coding(1) + size(2) = 8? Actually 4+4=8
SYNC_SIZE = 12

# Submode bits
SUBMODE_EOF = 0x80
SUBMODE_REALTIME = 0x40
SUBMODE_FORM2 = 0x20
SUBMODE_DATA = 0x10
SUBMODE_AUDIO = 0x08
SUBMODE_VIDEO = 0x04
SUBMODE_EOR = 0x02
SUBMODE_EOF_FILE = 0x01

# Channel assignments
CHANNEL_VIDEO = 0
CHANNEL_AUDIO = 1

def create_str_sector(file_num, channel, submode, data, is_eof=False):
    """Create a single STR sector (2352 bytes raw)."""
    sector = bytearray(SECTOR_SIZE)
    
    # Sync pattern (12 bytes): 00 FF FF FF FF FF FF FF FF FF FF 00
    sector[0] = 0x00
    sector[1:11] = b'\xFF' * 10
    sector[11] = 0x00
    
    # Subheader (4 bytes, repeated twice for error correction)
    # Byte 0-1: File number (16-bit)
    # Byte 2: Channel number
    # Byte 3: Submode
    # Byte 4-5: Coding info (for audio) / size for data
    # Byte 6-7: Same as 0-1 (duplicate)
    
    subheader = bytearray(8)
    subheader[0] = file_num & 0xFF
    subheader[1] = (file_num >> 8) & 0xFF
    subheader[2] = channel
    subheader[3] = submode
    subheader[4] = 0  # coding info
    subheader[5] = 0  # coding info
    subheader[6] = file_num & 0xFF
    subheader[7] = (file_num >> 8) & 0xFF
    
    sector[12:20] = subheader
    
    # Data (2048 bytes)
    data_len = min(len(data), DATA_SIZE)
    sector[24:24+data_len] = data[:data_len]
    
    # Pad rest with zeros
    # EDC (4 bytes) at offset 2064 - we'll skip for now
    # ECC would be at 2068-2155 (88 bytes) - skipped in raw
    
    return bytes(sector)

def jpeg_to_mdec_data(jpeg_bytes):
    """Convert JPEG to MDEC-compatible data.
    The PS1 MDEC expects JPEG with specific quantization tables.
    For simplicity, we'll use the JPEG as-is and let MDEC decode it.
    """
    # The MDEC can decode baseline JPEG. We need to ensure:
    # - YCbCr 4:2:0
    # - Standard Huffman tables (or we upload custom ones)
    # - No restart markers
    return jpeg_bytes

def main():
    frames_dir = '/tmp'
    output_file = '/home/tonym/Projects/plattypus-psoxide/Videos/intro.str'
    
    frames = sorted([f for f in os.listdir(frames_dir) if f.startswith('intro_') and f.endswith('.jpg')])
    print(f'Processing {len(frames)} frames...')
    
    file_num = 0x0001  # File number for this STR
    
    with open(output_file, 'wb') as f:
        frame_count = 0
        for frame_file in frames:
            frame_path = os.path.join(frames_dir, frame_file)
            
            # Read JPEG frame
            with open(frame_path, 'rb') as jf:
                jpeg_data = jf.read()
            
            # Convert to MDEC data
            mdec_data = jpeg_to_mdec_data(jpeg_data)
            
            # Split into sectors (2048 bytes each)
            offset = 0
            total_len = len(mdec_data)
            is_first_sector = True
            
            while offset < total_len:
                chunk = mdec_data[offset:offset + DATA_SIZE]
                remaining = total_len - offset - len(chunk)
                
                # Build submode
                submode = SUBMODE_FORM2 | SUBMODE_DATA | SUBMODE_VIDEO | SUBMODE_REALTIME
                if is_first_sector:
                    submode |= SUBMODE_EOR  # End of record (frame start)
                if remaining <= 0:
                    submode |= SUBMODE_EOF  # End of file (last sector of frame)
                
                sector = create_str_sector(file_num, CHANNEL_VIDEO, submode, chunk, remaining <= 0)
                f.write(sector)
                
                offset += DATA_SIZE
                is_first_sector = False
            
            frame_count += 1
            if frame_count % 30 == 0:
                print(f'  Processed {frame_count} frames...')
        
        # Write EOF sector
        eof_sector = create_str_sector(file_num, CHANNEL_VIDEO, 
                                       SUBMODE_FORM2 | SUBMODE_DATA | SUBMODE_VIDEO | SUBMODE_EOF | SUBMODE_EOR,
                                       b'')
        f.write(eof_sector)
    
    print(f'Done! Created {output_file} ({os.path.getsize(output_file)} bytes, {os.path.getsize(output_file)//SECTOR_SIZE} sectors)')

if __name__ == '__main__':
    main()