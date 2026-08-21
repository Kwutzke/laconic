// a line comment

/* a block comment */

/**
 * Documents an exported class.
 */
public class Probe {
    /** Documents an exported method. */
    public int exported(int a) {
        int x = a + 1; // a trailing comment
        System.out.println(x);

        // a detached comment

        return x;
    }

    @SuppressWarnings("public-api")
    private void hidden() {}

    private static class Helper {}
}
