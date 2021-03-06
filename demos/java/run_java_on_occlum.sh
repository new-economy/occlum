#!/bin/bash
set -e

BLUE='\033[1;34m'
NC='\033[0m'

show_usage() {
    echo "Error: invalid arguments"
    echo "Usage: $0 web_app/hello"
    exit 1
}

check_file_exist() {
    file=$1
    if [ ! -f ${file} ];then
        echo "Error: cannot stat file '${file}'"
        echo "Please see README and build it"
        exit 1
    fi
}

init_workspace() {
    # Init Occlum Workspace
    rm -rf occlum_context && mkdir occlum_context
    cd occlum_context
    occlum init
    new_json="$(jq '.resource_limits.user_space_size = "1400MB" |
                .resource_limits.kernel_space_heap_size="64MB" |
                .resource_limits.max_num_of_threads = 64 |
                .process.default_heap_size = "256MB" |
                .process.default_mmap_size = "1120MB" |
                .entry_points = [ "/usr/lib/jvm/java-11-openjdk/jre/bin" ] |
                .env.default = [ "LD_LIBRARY_PATH=/usr/lib/jvm/java-11-openjdk/jre/lib/server:/usr/lib/jvm/java-11-openjdk/jre/lib:/usr/lib/jvm/java-11-openjdk/jre/../lib" ]' Occlum.json)" && \
    echo "${new_json}" > Occlum.json
}

build_web() {
    # Copy JVM and JAR file into Occlum Workspace and build
    mkdir -p image/usr/lib
    cp -r /opt/occlum/toolchains/jvm image/usr/lib/
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    mkdir -p image/usr/lib/spring
    cp ../${jar_path} image/usr/lib/spring/
    occlum build
}

run_web() {
    jar_path=./gs-messaging-stomp-websocket/complete/target/gs-messaging-stomp-websocket-0.1.0.jar
    check_file_exist ${jar_path}
    jar_file=`basename "${jar_path}"`
    init_workspace
    build_web
    echo -e "${BLUE}occlum run JVM web app${NC}"
    occlum run /usr/lib/jvm/java-11-openjdk/jre/bin/java -Xmx512m -XX:MaxMetaspaceSize=64m -Dos.name=Linux -jar /usr/lib/spring/${jar_file}
}

build_hello() {
    # Copy JVM and class file into Occlum Workspace and build
    mkdir -p image/usr/lib
    cp -r /opt/occlum/toolchains/jvm image/usr/lib/
    cp /usr/local/occlum/x86_64-linux-musl/lib/libz.so.1 image/lib
    cp ../${hello} image
    occlum build
}

run_hello() {
    hello=./hello_world/Main.class
    check_file_exist ${hello}
    init_workspace
    build_hello
    echo -e "${BLUE}occlum run JVM hello${NC}"
    occlum run /usr/lib/jvm/java-11-openjdk/jre/bin/java -Xmx512m -XX:MaxMetaspaceSize=64m -Dos.name=Linux Main
}

arg=$1
case "$arg" in
    web_app)
        run_web
        ;;
    hello)
        run_hello
        ;;
    *)
        show_usage
esac
