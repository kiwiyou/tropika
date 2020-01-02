FROM rust@sha256:89e6e13d12a8bc0c76a014737b9b2758d2e5bba5571522bc46007f28af7ea059 as build
WORKDIR /usr/src/tropika
COPY . .

RUN cargo build --release
RUN mkdir -p /build-out
RUN cp target/release/tropika log4rs.yml /build-out/

FROM ubuntu@sha256:2695d3e10e69cc500a16eae6d6629c803c43ab075fa5ce60813a0fc49c47e859

ENV DEBIAN_FRONTEND=noninteractive
RUN apt-get update && apt-get -y install ca-certificates libssl-dev firejail build-essential curl python3
RUN curl -sL https://deb.nodesource.com/setup_10.x | bash && apt-get -y install nodejs && rm -rf /var/lib/apt/lists/*

COPY --from=build /build-out/* /
CMD /tropika