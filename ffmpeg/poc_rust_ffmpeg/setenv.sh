#!/bin/bash


# export FFMPEG_DIR="$( cd "$( dirname $0)/../stage" && pwd )"
export FFMPEG_DIR=$( cd -- "$( dirname -- "${BASH_SOURCE[0]}" )/../stage" &> /dev/null && pwd )
echo "export FFMPEG_DIR=$FFMPEG_DIR"

# CURR_DIR="$( cd "$( dirname $0)" && pwd )"
# FFMPEG_DIR="$CURR_DIR/../stage"
# FFMPEG_DIR="$( cd $FFMPEG_DIR && pwd )"
# export FFMPEG_DIR="$FFMPEG_DIR"
# echo "$FFMPEG_DIR"
