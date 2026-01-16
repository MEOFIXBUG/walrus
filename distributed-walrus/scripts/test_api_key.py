#!/usr/bin/env python3
"""
Test script for API KEY authentication
"""
import socket
import struct
import sys

def send_command(host, port, command, api_key=None):
    """Send a command to Walrus server"""
    try:
        sock = socket.socket(socket.AF_INET, socket.SOCK_STREAM)
        sock.settimeout(5)
        sock.connect((host, port))
        
        # If API key is provided, authenticate first
        if api_key:
            auth_cmd = f"AUTH {api_key}"
            send_frame(sock, auth_cmd)
            response = recv_frame(sock)
            if response != "OK":
                print(f"❌ Authentication failed: {response}")
                sock.close()
                return None
            print(f"✅ Authenticated with API key")
        
        # Send the actual command
        send_frame(sock, command)
        response = recv_frame(sock)
        sock.close()
        return response
    except Exception as e:
        print(f"❌ Error: {e}")
        return None

def send_frame(sock, message):
    """Send a length-prefixed frame"""
    data = message.encode('utf-8')
    length = len(data)
    sock.sendall(struct.pack('<I', length))
    sock.sendall(data)

def recv_frame(sock):
    """Receive a length-prefixed frame"""
    length_bytes = sock.recv(4)
    if len(length_bytes) != 4:
        raise Exception("Failed to read length")
    length = struct.unpack('<I', length_bytes)[0]
    data = sock.recv(length)
    if len(data) != length:
        raise Exception("Failed to read complete message")
    return data.decode('utf-8')

def main():
    host = "127.0.0.1"
    port = 9091
    api_key = "walrus-secret-key-123"
    
    print("=" * 60)
    print("Testing Walrus API KEY Authentication")
    print("=" * 60)
    
    # Test 1: Try without API key (should fail)
    print("\n1. Testing without API key (should fail):")
    response = send_command(host, port, "REGISTER test-topic")
    if response and "authentication required" in response.lower():
        print(f"✅ Correctly rejected: {response}")
    else:
        print(f"❌ Unexpected response: {response}")
    
    # Test 2: Try with wrong API key (should fail)
    print("\n2. Testing with wrong API key (should fail):")
    response = send_command(host, port, "REGISTER test-topic", api_key="wrong-key")
    if response and ("invalid" in response.lower() or "authentication" in response.lower()):
        print(f"✅ Correctly rejected: {response}")
    else:
        print(f"❌ Unexpected response: {response}")
    
    # Test 3: Authenticate and use commands
    print("\n3. Testing with correct API key:")
    response = send_command(host, port, "REGISTER test-topic", api_key=api_key)
    if response == "OK":
        print(f"✅ Topic registered: {response}")
    else:
        print(f"❌ Failed to register topic: {response}")
        return
    
    # Test 4: PUT with authentication
    print("\n4. Testing PUT with authentication:")
    response = send_command(host, port, "PUT test-topic hello-world", api_key=api_key)
    if response == "OK":
        print(f"✅ Data written: {response}")
    else:
        print(f"❌ Failed to write: {response}")
        return
    
    # Test 5: GET with authentication
    print("\n5. Testing GET with authentication:")
    response = send_command(host, port, "GET test-topic", api_key=api_key)
    if response and response.startswith("OK "):
        data = response[3:]
        print(f"✅ Data read: {data}")
        if data == "hello-world":
            print("✅ Data matches!")
        else:
            print(f"❌ Data mismatch: expected 'hello-world', got '{data}'")
    else:
        print(f"❌ Failed to read: {response}")
    
    # Test 6: STATE with authentication
    print("\n6. Testing STATE with authentication:")
    response = send_command(host, port, "STATE test-topic", api_key=api_key)
    if response and not response.startswith("ERR"):
        print(f"✅ Topic state retrieved")
        print(f"   State: {response[:100]}...")
    else:
        print(f"❌ Failed to get state: {response}")
    
    print("\n" + "=" * 60)
    print("✅ All API KEY tests completed!")
    print("=" * 60)

if __name__ == "__main__":
    main()

