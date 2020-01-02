FROM rust:latest as build
WORKDIR /usr/src/tropika
COPY . .

RUN cargo build --release
RUN mkdir -p /build-out
RUN cp target/release/tropika log4rs.yml /build-out/

FROM bitnami/minideb:latest

RUN install_packages ca-certificates libssl-dev firejail g++ nodejs python3
RUN useradd -ms /bin/bash bot
USER bot
WORKDIR /home/bot
COPY --from=build /build-out/* ./
CMD ./tropika