#!/bin/bash

openssl req -x509 -newkey rsa:4096 -keyout glauth.key -out glauth.crt -days 36500 -nodes -subj '/CN=localhost'

