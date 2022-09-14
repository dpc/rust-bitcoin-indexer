SHELL := bash
APP_NAME = madrid
PROJECT_ROOT = $(WELLE_ROOT)/$(APP_NAME)
PROTO_REPO = $(WELLE_ROOT)/proto
GCP_PROD_PROJECT_ID = welle-prod

default: proto/gen lint

start/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid server"

start/admin/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid admin-server"

start/listen-swap/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-swap"

start/listen-swap/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-swap --process-errors"

start/listen-withdraw/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-withdraw"

start/listen-withdraw/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-withdraw --process-errors"

start/listen-product-deposit/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-product-deposit"

start/listen-product-deposit/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-product-deposit --process-errors"

start/listen-product-withdraw/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-product-withdraw"

start/listen-product-withdraw/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-product-withdraw --process-errors"

start/chain-scanner/polygon/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid chain-scanner BLOCKCHAIN_POLYGON"

start/chain-scanner/ethereum/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid chain-scanner BLOCKCHAIN_ETHEREUM"

start/chain-scanner/avalanche/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid chain-scanner BLOCKCHAIN_AVALANCHE"

start/chain-scanner/solana/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid chain-scanner BLOCKCHAIN_SOLANA"

start/product-onchain-metrics-sync/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid product-onchain-metrics-sync"

start/product-balance-sync/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid product-balance-sync"

start/user-balance-metrics-sync/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid user-balance-metrics-sync"

start/balance-integrity-sync/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid balance-integrity-sync"

start/pending-events-sync/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid pending-events-sync"

start/listen-forward-swap/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-forward-swap"

start/listen-forward-swap/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-forward-swap --process-errors"

start/listen-transfer/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-transfer"

start/listen-transfer/process-errors/staging:
	npx nodemon --exec "APP_CONFIG_PATH=./staging.app.json bin/madrid listen-transfer --process-errors"

migrations/run:
	APP_CONFIG_PATH=./staging.app.json npx ts-node src/migration.ts

install/pre:
	rm -rf node_modules
	npm install -g google-artifactregistry-auth
	npx google-artifactregistry-auth

install: install/pre
	yarn

mac/install: install/pre
	sh yarn-mac.sh
	yarn postinstall

lint: lint/gql
	yarn tsc
	yarn run eslint "src/**/*.ts" --fix


lint/ci: lint/gql
	yarn tsc
	yarn run eslint "src/**/*.ts" --max-warnings=0

lint/gql:
	cat federation.graphql src/publicgraph/public.graphql | yarn run graphql-schema-linter -s
	cat federation.graphql src/admingraph/admin.graphql | yarn run graphql-schema-linter -s

k8s/context/staging:
	@gcloud config set project $(GCP_STAGING_PROJECT_ID)
	@gcloud container clusters get-credentials staging-gke-1 --region us-east1

k8s/trigger/product-onchain-metrics-sync/staging: k8s/context/staging
	kubectl create job --from=cronjob/product-onchain-metrics-sync product-onchain-metrics-sync-oneoff --namespace=$(APP_NAME)

k8s/trigger/user-balance-metrics-sync/staging: k8s/context/staging
	kubectl create job --from=cronjob/user-balance-metrics-sync user-balance-metrics-sync-oneoff --namespace=$(APP_NAME)

k8s/logs/server/staging: k8s/context/staging
	kubectl logs deployment/$(APP_NAME)-server --namespace=$(APP_NAME) main --follow

k8s/logs/worker/listen-withdraw/staging: k8s/context/staging
	kubectl logs deployment/$(APP_NAME)-worker --namespace=$(APP_NAME) listen-withdraw --follow

k8s/logs/worker/listen-swap/staging: k8s/context/staging
	kubectl logs deployment/$(APP_NAME)-worker --namespace=$(APP_NAME) listen-swap --follow

k8s/logs/worker/listen-product-deposit/staging: k8s/context/staging
	kubectl logs deployment/$(APP_NAME)-worker --namespace=$(APP_NAME) listen-product-deposit --follow

k8s/logs/worker/listen-product-withdraw/staging: k8s/context/staging
	kubectl logs deployment/$(APP_NAME)-worker --namespace=$(APP_NAME) listen-product-withdraw --follow

k8s/pods/staging: k8s/context/staging
	kubectl get pods --namespace=$(APP_NAME)

k8s/events/staging: k8s/context/staging
	kubectl get events --sort-by='.lastTimestamp' --namespace=$(APP_NAME)

k8s/service/staging: k8s/context/staging
	kubectl get service --namespace=$(APP_NAME)

image/build/staging:
	$(eval COMMIT_HASH := $(shell git rev-parse --short=5 HEAD))
	@docker build -t $(APP_NAME) .
	@docker tag $(APP_NAME):latest us-east1-docker.pkg.dev/$(GCP_STAGING_PROJECT_ID)/staging-registry/$(APP_NAME):latest
	@docker tag $(APP_NAME):latest us-east1-docker.pkg.dev/$(GCP_STAGING_PROJECT_ID)/staging-registry/$(APP_NAME):$(COMMIT_HASH)

image/push/staging: image/build/staging
	@gcloud config set project $(GCP_STAGING_PROJECT_ID)
	@gcloud auth configure-docker us-east1-docker.pkg.dev --quiet
	@docker push us-east1-docker.pkg.dev/$(GCP_STAGING_PROJECT_ID)/staging-registry/$(APP_NAME):latest
	@docker push us-east1-docker.pkg.dev/$(GCP_STAGING_PROJECT_ID)/staging-registry/$(APP_NAME):$(COMMIT_HASH)

k8s/rollout/staging: image/push/staging
	@gcloud container clusters get-credentials staging-gke-1 --region us-east1
	kubectl rollout restart deployment $(APP_NAME)-server --namespace=$(APP_NAME)
	kubectl rollout restart deployment $(APP_NAME)-worker --namespace=$(APP_NAME)

k8s/bounce/server/staging: k8s/context/staging
	kubectl rollout restart deployment $(APP_NAME)-server --namespace=$(APP_NAME)

k8s/bounce/worker/staging: k8s/context/staging
	kubectl rollout restart deployment $(APP_NAME)-worker --namespace=$(APP_NAME)

k8s/managedcert/staging: k8s/context/staging
	kubectl describe managedcertificate staging-$(APP_NAME)-managed-cert --namespace=$(APP_NAME)

gcloud/sa/staging:
	if [ ! -f staging.sa.json ] ; \
	then \
		gcloud iam service-accounts keys create staging.sa.json \
			--iam-account=$(APP_NAME)-staging-sa@$(GCP_PROJECT_ID).iam.gserviceaccount.com ; \
	fi

proto/gen: mac/install
	@echo ">>>>> Checking proto repo for new or unsaved changes."
	@cd $(PROTO_REPO) && git pull --rebase
	@rm -rf src/proto/gen && mkdir src/proto/gen
	node bin/proto.js
	# These proto files are needed for GRPC reflection -- used by grpcurl.
	cp $(WELLE_ROOT)/proto/$(APP_NAME)/$(APP_NAME).proto ./src/proto/gen/$(APP_NAME).proto
	cp $(WELLE_ROOT)/proto/common.proto ./src/proto/gen/common.proto

test/pre: redis/start
	@docker stop $(APP_NAME)-testing-db;  docker rm $(APP_NAME)-testing-db; true;
	@docker run \
		--detach \
		--name=$(APP_NAME)-testing-db \
		--env="MYSQL_USER=user" \
		--env="MYSQL_PASSWORD=password" \
		--env="MYSQL_DATABASE=$(APP_NAME)" \
		--publish 4306:3306 \
		mysql/mysql-server:latest \
		mysqld --default-authentication-plugin=mysql_native_password
	@sleep 35

redis/start:
	@docker stop $(APP_NAME)-redis;  docker rm $(APP_NAME)-redis; true;
	@docker run \
		--detach \
		--env REDIS_PASSWORD=FqwgzZHn5I \
		--name=$(APP_NAME)-redis \
		--publish 7379:6379  \
		redis:latest

test/run:
	npx jest -i --coverage --verbose=true

image/build/prod:
	$(eval COMMIT_HASH := $(shell git rev-parse --short=5 HEAD))
	@docker build -t $(APP_NAME) --build-arg GITHUB_PAT="${GITHUB_PAT}" --build-arg COMMIT_HASH="$(COMMIT_HASH)" .
	@docker tag $(APP_NAME):latest us-east1-docker.pkg.dev/$(GCP_PROD_PROJECT_ID)/prod-registry/$(APP_NAME):latest
	@docker tag $(APP_NAME):latest us-east1-docker.pkg.dev/$(GCP_PROD_PROJECT_ID)/prod-registry/$(APP_NAME):$(COMMIT_HASH)

image/push/prod:
	$(eval COMMIT_HASH := $(shell git rev-parse --short=5 HEAD))
	@gcloud config set project $(GCP_PROD_PROJECT_ID)
	@gcloud auth configure-docker us-east1-docker.pkg.dev --quiet
	@docker push us-east1-docker.pkg.dev/$(GCP_PROD_PROJECT_ID)/prod-registry/$(APP_NAME):latest
	@docker push us-east1-docker.pkg.dev/$(GCP_PROD_PROJECT_ID)/prod-registry/$(APP_NAME):$(COMMIT_HASH)

push/prod:
	@git fetch origin production
	@git push
	@git checkout production
	@git reset --hard origin/main && git push --set-upstream origin production
	@git checkout main

k8s/context/prod:
	@gcloud config set project $(GCP_PROD_PROJECT_ID)
	@gcloud container clusters get-credentials prod-gke --region us-east1

k8s/logs/server/prod: k8s/context/prod
	kubectl logs -f deployment/$(APP_NAME)-server --namespace=$(APP_NAME) --follow

k8s/pods/prod: k8s/context/prod
	kubectl get pods --namespace=$(APP_NAME)

k8s/events/prod: k8s/context/prod
	kubectl get events --sort-by='.lastTimestamp' --namespace=$(APP_NAME)

k8s/service/prod: k8s/context/prod
	kubectl get service --namespace=$(APP_NAME)

k8s/bounce/server/prod: k8s/context/prod
	kubectl rollout restart deployment $(APP_NAME)-server --namespace=$(APP_NAME)

k8s/rollout/worker/prod: k8s/context/prod
	kubectl rollout restart deployment $(APP_NAME)-worker --namespace=$(APP_NAME)

federation/publish/staging:
	APOLLO_KEY=service:welle-api:9sGw8DeAN30blubmnGlJ-w rover subgraph publish welle-api@staging \
		--schema ./src/publicgraph/public.graphql \
		--name $(APP_NAME) \
		--routing-url http://$(APP_NAME).$(APP_NAME).svc.cluster.local:8080/graphql \
		--convert
	APOLLO_KEY=service:internal-staging:sDcZ5KLoGOWzIZjBsyxEdA rover subgraph publish internal-staging@current \
		--schema ./src/admingraph/admin.graphql \
		--name $(APP_NAME) \
		--routing-url http://$(APP_NAME).$(APP_NAME).svc.cluster.local:8081/admin/graphql \
		--convert

federation/publish/prod:
	rover subgraph publish welle-prod@current \
		--schema ./src/publicgraph/public.graphql \
		--name $(APP_NAME) \
		--routing-url http://$(APP_NAME).$(APP_NAME).svc.cluster.local:8080/graphql \
		--convert
	APOLLO_KEY=$(APOLLO_INTERNAL_PROD_KEY) rover subgraph publish internal-prod@current \
		--schema ./src/admingraph/admin.graphql \
		--name $(APP_NAME) \
		--routing-url http://$(APP_NAME).$(APP_NAME).svc.cluster.local:8081/admin/graphql \
		--convert