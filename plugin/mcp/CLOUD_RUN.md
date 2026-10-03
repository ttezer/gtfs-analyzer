# Cloud Run deployment runbook

Docker and `gcloud` are not available on the development Mac, so deployment is
intentionally manual and approval-gated. Run these commands from the repository
root in an authenticated Cloud Shell or CI runner.

## Build and push

Set the project, region, Artifact Registry repository, and image explicitly:

```bash
gcloud config set project PROJECT_ID
gcloud services enable run.googleapis.com cloudbuild.googleapis.com artifactregistry.googleapis.com
gcloud builds submit \
  --config plugin/mcp/cloudbuild.yaml \
  --substitutions=_IMAGE=REGION-docker.pkg.dev/PROJECT_ID/REPOSITORY/gtfs-validator:0.1.0 \
  .
```

The Artifact Registry repository must already exist in `REGION`.

## Deploy

The following values are provisional until the Linux memory benchmark is
complete. Concurrency is intentionally one because the native Analyzer can use
hundreds of MiB for a feed much smaller than its compressed ZIP.

```bash
gcloud run deploy gtfs-validator \
  --image REGION-docker.pkg.dev/PROJECT_ID/REPOSITORY/gtfs-validator:0.1.0 \
  --region REGION \
  --platform managed \
  --allow-unauthenticated \
  --memory 1Gi \
  --cpu 2 \
  --concurrency 1 \
  --max-instances 2 \
  --timeout 120s \
  --set-env-vars GTFS_MAX_DOWNLOAD_BYTES=20971520,GTFS_TOTAL_TIMEOUT_SECONDS=105,GTFS_ANALYZER_TIMEOUT_SECONDS=90,GTFS_MAX_CONCURRENT_ANALYSES=1,GTFS_ANALYZER_WEB_URL=https://ttezer.github.io/gtfs-analyzer/
```

After deployment, obtain the service URL:

```bash
gcloud run services describe gtfs-validator --region REGION --format='value(status.url)'
```

The MCP endpoint is that URL with `/mcp` appended. Verify it with an MCP
initialize request and `tools/list` before putting it into a plugin connection.

Do not put a signed ChatGPT download URL, API key, or other credential in the
image, manifest, or committed files.
