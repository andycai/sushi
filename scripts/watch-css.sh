#!/bin/bash

# 监控 input.css 并自动编译为 style.css
./temp/tailwindcss -i ./temp/input.css -o ./web/static/css/style.css --watch
