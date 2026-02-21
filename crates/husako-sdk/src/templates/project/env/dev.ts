import { build } from "husako";
import { nginx } from "../deployments/nginx";

build([nginx]);
