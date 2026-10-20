import { Navigate, type RouteObject } from "react-router";
import { AriaRouter } from "@/lib/AriaRouter";
import { detailScreens } from "./detailScreens";
import { GALLERY_PATH, HOME_PATH, navigation, type PageLoader } from "./navigation";
import { SessionRoot } from "./SessionRoot";

/**
 * A route whose screen module loads on first navigation. The router waits for it before it
 * changes the location, so the screen arrives with its heading in place.
 */
function lazyRoute(path: string, page: PageLoader): RouteObject {
  return { path, lazy: async () => ({ Component: await page() }) };
}

// `import.meta.env.DEV` is false in builds, so the gallery chunk is left out of the bundle.
const devRoutes: RouteObject[] = import.meta.env.DEV
  ? [
      lazyRoute(GALLERY_PATH, () =>
        import("@/features/gallery/ComponentGallery").then((m) => m.ComponentGallery),
      ),
    ]
  : [];

const screens = [...navigation.flatMap((section) => section.screens), ...detailScreens];

// The shell and the sign-in screens load with the app; every screen loads when first opened.
export const routes: RouteObject[] = [
  {
    element: <AriaRouter />,
    children: [
      {
        path: "/",
        element: <SessionRoot />,
        // Nothing to show while the first screen's module loads; it is local and takes milliseconds.
        hydrateFallbackElement: null,
        children: [
          { index: true, element: <Navigate to={HOME_PATH} replace /> },
          // The single Catalog screen became the Test catalog area's four directories.
          {
            path: "/configuration/catalog",
            element: <Navigate to="/configuration/orderables" replace />,
          },
          ...screens.map(({ path, page, tabbed }) =>
            lazyRoute(tabbed ? `${path}/:tab?` : path, page),
          ),
        ],
      },
      ...devRoutes,
    ],
  },
  { path: "*", element: <Navigate to="/" replace /> },
];
