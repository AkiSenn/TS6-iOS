package dev.tslib;

import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;

/**
 * A hierarchical channel tree, implemented in pure Java.
 *
 * <pre>{@code
 * ChannelTree tree = ChannelTree.fromChannels(client.getChannels());
 * System.out.println(tree.printTree());
 * }</pre>
 */
public class ChannelTree implements AutoCloseable {
    private final Map<Long, Channel> channels = new HashMap<>();
    private final List<Long> roots = new ArrayList<>();
    private final Map<Long, List<Long>> children = new HashMap<>();

    public ChannelTree() {}

    /**
     * Build a tree from an array of channels.
     */
    public static ChannelTree fromChannels(Channel[] channels) {
        ChannelTree tree = new ChannelTree();
        if (channels != null) {
            for (Channel ch : channels) {
                tree.addChannel(ch);
            }
        }
        return tree;
    }

    private void addChannel(Channel ch) {
        channels.put(ch.id, ch);
        if (ch.parentId == 0) {
            roots.add(ch.id);
        } else {
            children.computeIfAbsent(ch.parentId, k -> new ArrayList<>()).add(ch.id);
        }
    }

    /**
     * Sort a list of channel IDs according to the TS3 linked-list order field.
     * Each channel's {@code order} is the ID of the channel it comes after (0 = first).
     */
    private List<Long> sortByOrder(List<Long> ids) {
        if (ids == null || ids.size() <= 1) return ids;

        // Build afterId → channelId map
        Map<Long, Long> afterMap = new HashMap<>();
        for (Long id : ids) {
            Channel ch = channels.get(id);
            if (ch != null) {
                afterMap.put((long) ch.order, id);
            }
        }

        List<Long> sorted = new ArrayList<>(ids.size());
        // Start from the channel that comes after 0 (first in the list)
        Long current = afterMap.get(0L);
        while (current != null && sorted.size() < ids.size()) {
            sorted.add(current);
            current = afterMap.get(current);
        }

        // Append any orphans not reached by the chain
        if (sorted.size() < ids.size()) {
            for (Long id : ids) {
                if (!sorted.contains(id)) {
                    sorted.add(id);
                }
            }
        }

        return sorted;
    }

    /**
     * Get root channels (those with no parent), sorted by order.
     */
    public Channel[] getRoots() {
        return sortByOrder(roots).stream()
                .map(channels::get)
                .toArray(Channel[]::new);
    }

    /**
     * Get the children of a channel, sorted by order.
     */
    public Channel[] getChildren(long parentId) {
        List<Long> ids = children.get(parentId);
        if (ids == null) return new Channel[0];
        return sortByOrder(ids).stream()
                .map(channels::get)
                .toArray(Channel[]::new);
    }

    /**
     * Get the path from root to the specified channel.
     */
    public Channel[] pathTo(long id) {
        List<Channel> path = new ArrayList<>();
        long current = id;
        while (current != 0 && channels.containsKey(current)) {
            Channel ch = channels.get(current);
            path.add(0, ch);
            current = ch.parentId;
        }
        return path.toArray(new Channel[0]);
    }

    /**
     * Find a channel by name (case-sensitive).
     *
     * @return the channel, or {@code null} if not found
     */
    public Channel findByName(String name) {
        for (Channel ch : channels.values()) {
            if (ch.name.equals(name)) return ch;
        }
        return null;
    }

    /**
     * Number of channels in the tree.
     */
    public int size() {
        return channels.size();
    }

    /**
     * Print an ASCII representation of the tree.
     */
    public String printTree() {
        StringBuilder sb = new StringBuilder();
        for (Long rootId : roots) {
            printNode(sb, rootId, "", true);
        }
        return sb.toString();
    }

    private void printNode(StringBuilder sb, long id, String prefix, boolean last) {
        Channel ch = channels.get(id);
        if (ch == null) return;

        sb.append(prefix);
        sb.append(last ? "└── " : "├── ");
        sb.append(ch.name).append("\n");

        List<Long> kids = children.get(id);
        if (kids != null) {
            String childPrefix = prefix + (last ? "    " : "│   ");
            for (int i = 0; i < kids.size(); i++) {
                printNode(sb, kids.get(i), childPrefix, i == kids.size() - 1);
            }
        }
    }

    @Override
    public void close() {
        channels.clear();
        roots.clear();
        children.clear();
    }

    @Override
    public String toString() {
        return "ChannelTree(channels=" + channels.size() + ")";
    }
}
