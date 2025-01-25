#!/bin/sh
set -euo

pnpm install --ignore-scripts
pnpm run asc $@
pnpm run asc:debug