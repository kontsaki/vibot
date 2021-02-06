#!/bin/bash
if [[ "$1" = "build" ]]; then
    docker build . -t vibot;
else
    # cargo bug
    github_ip=$(dig github.com -4 +short|head -n1)
    docker run \
    -it \
    --name vibot \
    -v `pwd`:/opt/vibot/ \
    -w /opt/vibot/ \
    --network bridge \
    --add-host github.com:$github_ip \
    vibot;
fi
