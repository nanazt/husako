import { deployment } from "k8s/apps/v1";
import { container } from "k8s/core/v1";
import { selector } from "k8s/_common";
import { cpu, memory, requests, limits } from "husako";
import { appMetadata } from "../lib";

export const nginx = deployment()
  .metadata(appMetadata("nginx"))
  .replicas(1)
  .selector(selector().matchLabels({ app: "nginx" }))
  .containers([
    container()
      .name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);
