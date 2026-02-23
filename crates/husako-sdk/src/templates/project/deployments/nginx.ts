import { Deployment } from "k8s/apps/v1";
import { Container } from "k8s/core/v1";
import { LabelSelector } from "k8s/_common";
import { cpu, memory, requests, limits } from "husako";
import { appMetadata } from "../lib";

export const nginx = Deployment()
  .metadata(appMetadata("nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }))
  .containers([
    Container()
      .name("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi"))
          .limits(cpu("500m").memory("256Mi"))
      )
  ]);
