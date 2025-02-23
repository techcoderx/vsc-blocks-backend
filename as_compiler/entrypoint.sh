#!/bin/sh
set -euo

cd compiler
rm -rf build/*

pnpm install --ignore-scripts
pnpm run asc $@
pnpm run asc:debug