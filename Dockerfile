FROM rust:latest
RUN apt-get update && apt-get install -y z3 build-essential clang
WORKDIR /usr/src/myapp
COPY . .
RUN cargo install --path .
ENTRYPOINT t6
