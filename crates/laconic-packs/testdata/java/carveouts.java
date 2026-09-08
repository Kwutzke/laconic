// CHECKSTYLE:OFF
// SUPPRESS CHECKSTYLE HiddenField
// NOPMD
// NOSONAR
// @formatter:off
// spotless:off

/**
 * Documents an exported class.
 */
public class Probe {
    private int count = 0; // $NON-NLS-1$

    // $NON-NLS-2$
    private String label = "x";

    /** Documents an exported method. */
    public int exported(int a) {
        int x = a + 1;
        return x;
    }

    @SuppressWarnings("public-api")
    private void hidden() {}

    private static class Helper {}

    interface Shape {}
}
