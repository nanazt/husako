import * as husako from "husako";
import { nginx } from "../base/nginx";
import { nginxService } from "../base/service";

husako.build([nginx("staging", 2, "nginx:1.25"), nginxService("staging")]);
