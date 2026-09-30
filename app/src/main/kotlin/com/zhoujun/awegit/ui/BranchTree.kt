package com.zhoujun.awegit.ui

import com.zhoujun.awegit.domain.models.Branch

sealed interface RefTreeNode {
    data class Folder(val name: String, val path: String, val children: List<RefTreeNode>) : RefTreeNode
    data class Leaf(val branch: Branch, val label: String) : RefTreeNode
}

fun buildRefTree(branches: List<Branch>, labelOf: (Branch) -> String = { it.simpleName }): List<RefTreeNode> {
    return buildLevel(branches, labelOf, "")
}

private fun buildLevel(branches: List<Branch>, labelOf: (Branch) -> String, prefix: String): List<RefTreeNode> {
    val grouped = linkedMapOf<String, MutableList<Branch>>()
    for (branch in branches) {
        val label = labelOf(branch)
        val rest = if (prefix.isEmpty()) label else label.removePrefix("$prefix/")
        val segment = rest.substringBefore('/')
        grouped.getOrPut(segment) { mutableListOf() }.add(branch)
    }
    return grouped.map { (segment, group) ->
        val path = if (prefix.isEmpty()) segment else "$prefix/$segment"
        val nested = group.filter { labelOf(it) != path && labelOf(it).startsWith("$path/") }
        val exact = group.filter { labelOf(it) == path }
        if (nested.isEmpty()) {
            val branch = exact.firstOrNull() ?: group.first()
            RefTreeNode.Leaf(branch, segment)
        } else {
            val children = buildLevel(nested, labelOf, path).toMutableList()
            exact.forEach { children.add(0, RefTreeNode.Leaf(it, segment)) }
            RefTreeNode.Folder(segment, path, children)
        }
    }
}
