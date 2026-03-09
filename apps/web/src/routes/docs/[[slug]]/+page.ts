import { error } from "@sveltejs/kit";

import { getDocPage, getNavSections } from "$lib/docs/content";

export const load = ({ params }) => {
    const doc = getDocPage(params.slug);

    if (!doc) {
        throw error(404, "Documentation page not found");
    }

    return {
        doc,
        navSections: getNavSections(),
    };
};
