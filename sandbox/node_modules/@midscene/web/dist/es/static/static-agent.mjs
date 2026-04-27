import { Agent } from "@midscene/core/agent";
class StaticPageAgent extends Agent {
    constructor(page){
        super(page, {
            generateReport: false
        });
        this.dryMode = true;
    }
}
export { StaticPageAgent };

//# sourceMappingURL=static-agent.mjs.map