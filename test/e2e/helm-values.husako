import husako from "husako";
import { LocalChart } from "helm/local-chart";

const chart = LocalChart()
  .replicaCount(2)
  .image({ repository: "nginx", tag: "1.25" });

husako.build([chart]);
