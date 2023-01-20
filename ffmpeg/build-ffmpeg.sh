#!/usr/bin/env bash

# echo "$(pwd)"
CMD0="$0"
CURR_DIR="$( cd "$( dirname $0)" && pwd )"
FFMPEG_WORK_DIR=${FFMPEG_WORK_DIR:-"${CURR_DIR}/target"}
FFMPEG_STAGE_DIR=${FFMPEG_STAGE_DIR:-"${FFMPEG_WORK_DIR}/stage"}

FFMPEG_URL="https://ffmpeg.org/releases/ffmpeg-4.4.3.tar.xz"
FFMPEG_TAR="${FFMPEG_WORK_DIR}/ffmpeg-4.4.3.tar.xz"
FFMPEG_SRC="${FFMPEG_WORK_DIR}/ffmpeg-4.4.3"

mkdir -p ${FFMPEG_WORK_DIR}
cd "${FFMPEG_WORK_DIR}"

function help {
    echo "Usage:"
    echo "  $CMD0 [Command]"
    echo "Command:"
    echo "  build       default. Auto fetching, unpacking, and building"
    echo "  build_once  build only once"
    echo "  rebuild     force rebuild by delete src/"
    echo "  fetch       downloading .tar"
    echo "  clean       delete src/ and stage/"
    echo "  clean_all   delete src/, stage/ and .tar "
}


function exit_msg {
    echo "failed: $@" >&2
    exit 1
}

function fail2die {
    "$@"
    local status=$?
    if [ $status -ne 0 ]; then
        echo "failed: $@" >&2
        exit 1
    fi
}

# todo: dont repeat the ffmpeg configure options list
function build {

    if [[ ! -f "$FFMPEG_TAR" ]]; then
        echo "none exist ffmpeg tar $FFMPEG_TAR"
        fetch
    fi

    if [[ ! -d "$FFMPEG_SRC" ]]; then
        echo "none exist ffmpeg src $FFMPEG_SRC"
        unpack
    fi

    local ffmpeg_src_path=$FFMPEG_SRC
    if  [ ! "$ffmpeg_src_path" ] ;then
        exit_msg "empty ffmpeg_src_path"
    fi

    local ffmpeg_stage_path=$FFMPEG_STAGE_DIR
    if  [ ! "$ffmpeg_stage_path" ] ;then
        exit_msg "empty ffmpeg_stage_path"
    fi
    mkdir -p $ffmpeg_stage_path

    echo "building ffmpeg: $ffmpeg_src_path"

    cd "$ffmpeg_src_path"

    # echo  "FORCE_REBUILD=$FORCE_REBUILD"

    # if  [ ! "$FORCE_REBUILD" ] ;then 
    #     echo "FORCE_REBUILD is null"
    # else
    #     echo "FORCE_REBUILD is set"
    #     local rebuild=$FORCE_REBUILD
    # fi
    
    if [[ ! -f "$ffmpeg_src_path/config.h" ]]; then
        echo "ffmpeg config.h NOT exist"
        local rebuild="1"
    fi

    # if [[ ! -f "$ffmpeg_src_path/config.mak" ]]; then
    if  [ ! "$rebuild" ] ;then 
        echo "skip configuring ffmpeg"
    else
        echo "configuring ffmpeg..."

	     # --pkg-config=$(which pkg-config) --pkg-config-flags='--static' \
	     # --pkg-config-flags='--static' \
	    # --extra-libs='-lopus' \
        FF_CMD="./configure --prefix=$ffmpeg_stage_path \
            --extra-cflags=-I$ffmpeg_stage_path/include --extra-ldflags=-L$ffmpeg_stage_path/lib \
   	        --enable-static --disable-shared \
            --disable-everything \
            --disable-debug \
	        --disable-doc --disable-protocols --disable-devices \
            --disable-audiotoolbox --disable-videotoolbox \
            --enable-protocol=file \
            --enable-hardcoded-tables \
            --enable-avcodec \
            --enable-avformat \
            --enable-avutil \
            --disable-swscale \
            --disable-avdevice \
            --disable-swresample \
            --disable-postproc \
            --disable-avfilter \
            --enable-lzo  \
            --enable-decoder=h264 \
            --enable-pic \
            --disable-doc \
            --disable-programs \
            "
        # FF_CMD="$FF_CMD \
        #     --enable-gpl \
        #     --enable-nonfree \
        #     --enable-libvpx --enable-encoder=libvpx_vp8 --enable-decoder=vp8 --enable-parser=vp8 \
        #     --enable-muxer=webm --enable-demuxer=matroska,webm --enable-muxer=matroska \
        #     --enable-libopus --enable-encoder=libopus --enable-decoder=libopus --enable-muxer=opus \
        #     --enable-muxer=wav \
	    #     --enable-demuxer=wav \
	    #     --enable-encoder=pcm_alaw --enable-decoder=pcm_alaw \
	    #     --enable-encoder=pcm_mulaw --enable-decoder=pcm_mulaw \
	    #     --enable-encoder=pcm_s8 --enable-decoder=pcm_s8  \
        #     --enable-encoder=pcm_s16le --enable-decoder=pcm_s16le --enable-encoder=pcm_s16be --enable-decoder=pcm_s16be \
        #     --enable-encoder=pcm_s24le --enable-decoder=pcm_s24le --enable-encoder=pcm_s24be --enable-decoder=pcm_s24be \
        #     --enable-encoder=pcm_s32le --enable-decoder=pcm_s32le --enable-encoder=pcm_s32be --enable-decoder=pcm_s32be \
	    #     --enable-encoder=pcm_s8_planar --enable-decoder=pcm_s8_planar  \
        #     --enable-encoder=pcm_s16le_planar --enable-decoder=pcm_s16le_planar --enable-encoder=pcm_s16be_planar --enable-decoder=pcm_s16be_planar \
        #     --enable-encoder=pcm_s24le_planar --enable-decoder=pcm_s24le_planar --enable-encoder=pcm_s24be_planar --enable-decoder=pcm_s24be_planar \
        #     --enable-encoder=pcm_s32le_planar --enable-decoder=pcm_s32le_planar --enable-encoder=pcm_s32be_planar --enable-decoder=pcm_s32be_planar \
        #     --enable-encoder=pcm_u8 --enable-decoder=pcm_u8 \
        #     --enable-encoder=pcm_u16le --enable-decoder=pcm_u16le --enable-encoder=pcm_u16be --enable-decoder=pcm_u16be \
        #     --enable-encoder=pcm_u24le --enable-decoder=pcm_u24le --enable-encoder=pcm_u24be --enable-decoder=pcm_u24be \
        #     --enable-encoder=pcm_u32le --enable-decoder=pcm_u32le --enable-encoder=pcm_u32be --enable-decoder=pcm_u32be \
        #     --enable-encoder=pcm_f16le --enable-decoder=pcm_f16le --enable-encoder=pcm_f16be --enable-decoder=pcm_f16be \
        #     --enable-encoder=pcm_f24le --enable-decoder=pcm_f24le --enable-encoder=pcm_f24be --enable-decoder=pcm_f24be \
        #     --enable-encoder=pcm_f32le --enable-decoder=pcm_f32le --enable-encoder=pcm_f32be --enable-decoder=pcm_f32be \
        #     --enable-encoder=pcm_f64le --enable-decoder=pcm_f64le --enable-encoder=pcm_f64be --enable-decoder=pcm_f64be \
        #     --enable-muxer=mjpeg \
        #     --enable-encoder=jpeg2000 \
        #     --disable-sdl2 --disable-securetransport --disable-iconv \
        #     --enable-libx264 --enable-encoder=libx264 --enable-decoder=h264 --enable-parser=h264 --enable-muxer=mp4 --enable-demuxer=mov,mp4,m4a,3gp,3g2,mj2,image2 \
        #     --enable-encoder=aac --enable-decoder=aac \
        #     --enable-filter=adelay --enable-filter=amix --enable-filter=amerge --enable-filter=anull \
        #     --enable-filter=asplit --enable-filter=apad --enable-filter=atrim --enable-filter=aevalsrc \
        #     --enable-filter=color --enable-filter=overlay --enable-filter=fifo --enable-filter=scale \
        #     --enable-filter=hstack --enable-filter=vstack --enable-filter=afifo \
        #     --enable-filter=null --enable-filter=pad --enable-filter=concat --enable-filter=aresample --enable-decoder=vorbis --enable-encoder=vorbis \
        #     --enable-filter=pan --enable-filter=dynaudnorm \
        #     --enable-encoder=rawvideo --enable-decoder=rawvideo --enable-muxer=rawvideo --enable-demuxer=rawvideo,image2 \
        #     --enable-filter=drawtext \
        #     --enable-filter=movie --enable-filter=select --enable-encoder=mjpeg,png --enable-decoder=mjpeg,png \
        #     --enable-libmp3lame \
        #     --enable-encoder=libmp3lame --enable-decoder=mp3 --enable-muxer=mp3 --enable-demuxer=mp3"
        #    # --enable-libfontconfig --enable-libfreetype --enable-libfribidi
           fail2die $FF_CMD
           make clean
    fi
    fail2die make -j4
    fail2die make install
    echo "builded ffmpeg: $ffmpeg_stage_path"
}




function clean_all {
    echo "deleting tar $FFMPEG_TAR"
    rm -rf $FFMPEG_TAR
    clean
}

function clean {
    echo "deleting src $FFMPEG_SRC"
    rm -rf $FFMPEG_SRC

    echo "deleting stage $FFMPEG_SRC"
    rm -rf $FFMPEG_STAGE_DIR
}

function fetch {
    echo "fetching ffmpeg: $FFMPEG_URL"
    # wget $FFMPEG_URL
    curl $FFMPEG_URL --output $FFMPEG_TAR
    echo "saved to $FFMPEG_TAR"
}

function unpack {
    echo "unpacking tar $FFMPEG_TAR"
    tar xf $FFMPEG_TAR
}

function rebuild {
    echo rebuild
    clean
    # FORCE_REBUILD="1"
    build
}

# function build {
    
#     build_third_ffmpeg $FFMPEG_SRC $FFMPEG_STAGE_DIR
# }

function build_once {
    if [[ ! -d "$FFMPEG_STAGE_DIR" ]]; then
        echo "rebuild ffmpeg for none exist $FFMPEG_STAGE_DIR"
        rebuild
    else
        echo "skip build ffmpeg for exist $FFMPEG_STAGE_DIR"
    fi
}


RUN_FUN=${1:-"build"}
$RUN_FUN


