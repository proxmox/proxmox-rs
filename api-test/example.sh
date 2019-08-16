#!/bin/sh

# Example api-test client commands:
echo "Calling /api/1/greet:"
curl -XGET -H 'Content-type: application/json' \
  -d '{"person":"foo","message":"a message"}' \
  'http://127.0.0.1:3000/api/1/greet'
echo

echo "Calling /api/1/mount/rootfs"
# without the optional 'ro' field
curl -XPOST -H 'Content-type: application/json' \
  -d '{"entry":{"mount_type":"volume","source":"/source","destination":"/destination"}}' \
  'http://127.0.0.1:3000/api/1/mount/rootfs'
echo

echo "Calling /api/1/mount/rootfs again"
# with the optional 'ro' field
curl -XPOST -H 'Content-type: application/json' \
  -d '{"entry":{"mount_type":"volume","source":"/source","destination":"/destination","ro":true}}' \
  'http://127.0.0.1:3000/api/1/mount/rootfs'
echo
