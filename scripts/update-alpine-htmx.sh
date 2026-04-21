#!/bin/bash

curl -L https://unpkg.com/htmx.org@latest/dist/htmx.min.js -o web/static/js/htmx.min.js
curl -L https://unpkg.com/alpinejs@latest/dist/cdn.min.js -o web/static/js/alpine.min.js
# curl -L https://unpkg.com/daisyui@latest/dist/cdn.min.js -o temp/daisyui.js
# 下载 daisyui 的通用 JS 插件文件
curl -L https://github.com/saadeghi/daisyui/releases/download/v5.5.19/daisyui.js -o ./temp/daisyui.js
