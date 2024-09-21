"""
A python script that implements AWS signature v4 and is used to provide ground truth
for the tests

Vendored from https://github.com/aws-samples/sigv4-signing-examples/blob/main/no-sdk/python/main.py
"""
import datetime
import hashlib
import hmac
import requests
import os

# AWS access keys - use minio default for localdev / tests
access_key = 'V5NSAQUNLNZ5AP7VLLS6'
secret_key = 'bu0K3n0kEag8GKfckKPBg4Vu8O8EuYu2UO/wNfqI'

# Request parameters
method = 'GET'
service = 's3'
host = "localhost:9000"
region = 'us-east-1'
endpoint = '/private/example_1_cog_deflate.tif'

# Create a datetime object for signing
# If you test against minio, uncomment this
t = datetime.datetime.utcnow()
# Hardcoded timestamp to check test expected values
#t = datetime.datetime(2024, 9, 28)
amzdate = t.strftime('%Y%m%dT%H%M%SZ')
datestamp = t.strftime('%Y%m%d')

# Create the canonical request
canonical_uri = endpoint
canonical_querystring = ''
canonical_headers = 'host:' + host + '\n'
signed_headers = 'host'
payload_hash = hashlib.sha256(''.encode('utf-8')).hexdigest()
print("sha256 not hex %s" % hashlib.sha256(''.encode('utf-8')).digest())
canonical_request = (method + '\n' + canonical_uri + '\n' + canonical_querystring + '\n'
                     + canonical_headers + '\n' + signed_headers + '\n' + payload_hash)

print(f"\n{canonical_request=}\n")

# Create the string to sign
algorithm = 'AWS4-HMAC-SHA256'
credential_scope = datestamp + '/' + region + '/' + service + '/' + 'aws4_request'
string_to_sign = (algorithm + '\n' +  amzdate + '\n' +  credential_scope + '\n' +
                  hashlib.sha256(canonical_request.encode('utf-8')).hexdigest())

print(f"\n{string_to_sign=}\n")

def sign(key, msg):
    return hmac.new(key, msg.encode("utf-8"), hashlib.sha256).digest()

def getSignatureKey(key, dateStamp, regionName, serviceName):
    kDate = sign(("AWS4" + key).encode("utf-8"), dateStamp)
    kRegion = sign(kDate, regionName)
    kService = sign(kRegion, serviceName)
    kSigning = sign(kService, "aws4_request")
    return kSigning

# Sign the string
signing_key = getSignatureKey(secret_key, datestamp, region, service)
print(f"\n{signing_key=}\n")
signature = hmac.new(signing_key, (string_to_sign).encode('utf-8'), hashlib.sha256).hexdigest()
print(f"\n{signature=}\n")

# Add signing information to the request
authorization_header = (algorithm + ' ' + 'Credential=' + access_key + '/' + credential_scope + ', ' +
                        'SignedHeaders=' + signed_headers + ', ' + 'Signature=' + signature)

# Make the request
headers = {'host': host,
           'x-amz-date': amzdate,
           'authorization': authorization_header,
           #'Range': '0-16384'
           }
request_url = 'http://' + host + canonical_uri
print(f"\n{authorization_header=}\n")
print(f"{request_url=}")
response = requests.get(request_url, headers=headers, timeout=5)
response.raise_for_status()

print(response.text[:100])
