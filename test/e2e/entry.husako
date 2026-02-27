import husako from "husako";
import { Deployment } from "k8s/apps/v1";
import { LabelSelector } from "k8s/_common";
import { name, namespace, label } from "k8s/meta/v1";
import { name as containerName, cpu, memory, requests } from "k8s/core/v1";

const nginx = Deployment()
  .metadata(name("nginx").namespace("default").label("app", "nginx"))
  .replicas(1)
  .selector(LabelSelector().matchLabels({ app: "nginx" }))
  .containers([
    containerName("nginx")
      .image("nginx:1.25")
      .resources(
        requests(cpu("250m").memory("128Mi")),
      ),
  ]);

husako.build([nginx]);
