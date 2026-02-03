# Reflection Catalog

The Reflection Catalog is a centralized gRPC reflection service. It serves as a single source of truth for all service definitions (protos) hosted under the `ad-services.fnal.gov` domain.

## Why this exists

In our Kubernetes environment, we use path-based routing via Traefik. Tools like `grpcurl` fail to discover services because the standard gRPC reflection calls do not include our custom path-based routing metadata.

The Reflection Catalog solves this by:

1. Aggregating all `.proto` definitions from the [interface-definitions](https://github.com/fermi-ad/interface-definitions) repository.
2. Providing a dedicated endpoint for `grpc.reflection.v1alpha.ServerReflection` requests.
3. Allowing Traefik to route discovery requests to this catalog while routing functional RPC calls to their respective microservice pods.

## Note to developers

This project comes with a `devcontainer.json` file, which references a prebuilt development container that should have all the necessary tools for developing in Rust (including `protoc`). Please make use of this. Install the "Dev Containers" extension in VS Code and you should be prompted to reopen the project in the container. This will save you the headache of having to install things yourself and will enforce tool versions across different developer machines. 

## To get started

1. Reopen this project in the Dev Container.
2. Run `cargo build` to verify the environment.
3. The service expects a local clone of `interface-definitions` to be present at the path specified in the `PROTO_PATH` environment variable.

## Architecture & Automation

The service is written in Rust using Tonic.

* Git-Sync Sidecar: In production, a `git-sync` container runs in the same pod. It monitors the interface definitions repository and pulls changes into a shared volume.
* Dynamic Loading: The Rust service monitors the shared volume. When changes are detected, it re-compiles the file descriptor sets in-memory, ensuring the reflection API is always up to date without requiring a pod restart.
* Flux CD: Deployment and image updates are managed via Flux.
    * Deployment: `reflection-catalog-deployment.yaml`
    * Auto-updates: `reflection-catalog-autoupdate.yaml`

## Automation & Updates

The `grpc-reflection-catalog` stays in sync with the [interface-definitions](https://github.com/fermi-ad/interface-definitions) repository through a dual-layered automation strategy:

### 1. Development & Local Testing

The `.devcontainer` is configured to automatically initialize the `interface-definitions` submodule and install necessary tools (`protoc`, `grpcurl`). 
* Simply run `cargo run` and the catalog will serve your local submodule protos.

### 2. Production Sync

In the K8s cluster, the service uses two mechanisms to stay current:

* Live Sync: A `git-sync` sidecar container monitors the `main` branch of the definitions repo and syncs changes to a shared volume in real-time.
* Triggered Rebuilds: The definitions repository sends a `repository_dispatch` (via a GitHub App) to this repository whenever protos are updated. This triggers a GitHub Action to rebuild the Docker image and update the Flux `ImagePolicy`, ensuring the container is periodically refreshed with the latest baked-in definitions as a backup to the live sync.

### 3. Deployment Flow

1. Push to `interface-definitions`.
2. GitHub Action notifies `reflection-catalog`.
3. GitHub Action here builds/pushes a new image to `adregistry.fnal.gov`.
4. Flux CD detects the new image and rolls out the update to the `controls-appdev` namespace.

### Automation Flow

The catalog uses a "Notification & Pull" architecture to ensure zero-manual updates:

1. Change Trigger: A developer pushes a change to the `proto/` directory in the `interface-definitions` repository.
2. Notification: A GitHub Action in that repo uses a GitHub App token to send a `repository_dispatch` to this repository.
3. Image Build: This repository triggers a build, baking the latest protos into a new Docker image tag and pushing it to `adregistry.fnal.gov`.
4. GitOps Deployment: Flux CD detects the new image tag and updates the ACORN cluster deployment.
5. Runtime Freshness: While the pod is running, a `git-sync` sidecar continuously monitors the git repo. If a minor change occurs between image builds, the Rust service detects the file update on the shared volume and refreshes the reflection provider dynamically

## Usage

With this catalog running, you no longer need to provide local `.proto` files to `grpcurl`.

### List all services

```bash
grpcurl -plaintext ad-services.fnal.gov:443 list
```

### Describe a specific service

```bash
grpcurl -plaintext ad-services.fnal.gov:443 describe services.alarm_timers.AlarmTimerService
```

### Invoke a method

```bash
grpcurl -plaintext -d '{ "timerType": "TimerType_SNOOZE" }' \
  ad-services.fnal.gov:443 services.alarm_timers.AlarmTimerService/read
```

