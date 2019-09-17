FROM rust:slim-buster
WORKDIR /usr/src/test_chunks
COPY . /usr/src
RUN cargo build --release

FROM debian:buster
COPY --from=0 /usr/src/test_chunks/target/release/test_chunks /bin/test_chunks
RUN mkdir /output
ENTRYPOINT [ "/bin/test_chunks" ]