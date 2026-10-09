"""Publish a local tmp image through the execution-scoped host service."""
import json
import os
import sys
import urllib.request
from urllib.parse import urlsplit


def main():
    if len(sys.argv) != 2:
        raise ValueError("usage: python3 show.py LOCAL_TMP_IMAGE")
    endpoint = os.environ.get("CRABOT_SKILL_ENDPOINT", "")
    token = os.environ.get("CRABOT_SKILL_TOKEN", "")
    address = urlsplit(endpoint)
    if (address.scheme != "http" or address.hostname != "127.0.0.1"
            or not address.port or address.path != "/image"
            or address.username or address.password or address.query or address.fragment or not token):
        raise ValueError("image-view requires an approved Crabot shell execution")
    request = urllib.request.Request(endpoint, data=json.dumps({"path": sys.argv[1]}).encode(),
        headers={"Content-Type": "application/json", "Authorization": "Bearer " + token})
    # Never send the local credential through an HTTP proxy or a redirect.
    class NoRedirect(urllib.request.HTTPRedirectHandler):
        def redirect_request(self, *args, **kwargs):
            return None
    opener = urllib.request.build_opener(urllib.request.ProxyHandler({}), NoRedirect())
    with opener.open(request, timeout=30) as response:
        print(json.dumps(json.load(response), ensure_ascii=False))


if __name__ == "__main__":
    main()
