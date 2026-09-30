package com.zhoujun.awegit.data.git.log.graph

interface IGraphNode {
    val graphParentCount: Int
    fun getGraphParent(nth: Int): GraphNode
}