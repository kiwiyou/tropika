FROM rust:latest as build
WORKDIR /usr/src/tropika
COPY . .
RUN cargo build --release

FROM bitnami/minideb:latest

RUN install_packages ca-certificates libssl-dev \
    && useradd -ms /bin/bash bot
USER bot
WORKDIR /home/bot
COPY --from=build /usr/src/tropika/target/release/* ./
CMD ./tropika