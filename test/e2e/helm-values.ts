import { LocalChart } from "helm/local-chart";
import { build } from "husako";

const chart = LocalChart()
  .replicaCount(2)
  .image({ repository: "nginx", tag: "1.25" });

build([chart]);
